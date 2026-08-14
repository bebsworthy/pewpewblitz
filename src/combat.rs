//! The first authoritative combat slice: one direct-fire weapon, projectiles, damage, and reset.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::cast_precision_loss
)]

pub mod attack;
#[cfg(feature = "client")]
pub mod client;
pub mod definitions;
pub mod delivery;
pub mod effects;
#[cfg(feature = "server")]
pub mod server;
pub mod telemetry;

pub use definitions::{
    DamageFalloff, DeliveryMethod, EngineWeaponLimits, FiringPattern, GameplayContentFingerprint,
    PayloadBundleDefinition, PayloadEffectDefinition, RecipientPolicy, ResolvedWeapon,
    SlowStacking, TargetSelection, WeaponCatalog, WeaponCatalogResource, WeaponConfiguration,
    WeaponEconomy, WeaponPresentationProfileId, WeaponPresetDefinition, WeaponPresetId,
    WeaponRecipe, WeaponRecipeFingerprint, WeaponRecipePolicy, linear_falloff,
    resolve_configuration, spread_angles,
};
pub use telemetry::{
    CombatFighterSnapshot, CombatProjectileSnapshot, CombatStateSnapshot,
    WeaponSelectionTelemetryRecord, WeaponTelemetry, WeaponTelemetryAggregate, WeaponTelemetryKey,
    WeaponTelemetryOutcome, WeaponTelemetryRecord, encode_state_snapshot,
};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
#[cfg(any(feature = "server", feature = "client"))]
use std::collections::BTreeMap;
#[cfg(any(feature = "server", feature = "client"))]
use std::collections::HashMap;
use std::collections::HashSet;
#[cfg(feature = "client")]
use std::collections::VecDeque;
#[cfg(any(feature = "server", feature = "client"))]
use std::env;
#[cfg(feature = "client")]
use std::fmt::Write as _;
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(feature = "client")]
use std::{fs, path::PathBuf, time::Instant};

use avian2d::prelude::Position;
#[cfg(any(feature = "server", feature = "client"))]
use avian2d::prelude::Rotation;

#[cfg(feature = "server")]
use avian2d::prelude::{
    AngularVelocity, Collider, CollisionLayers, CustomPositionIntegration, LinearVelocity,
    RigidBody,
};
#[cfg(feature = "server")]
use lightyear::prelude::input::native::ActionState;
#[cfg(feature = "server")]
use lightyear::prelude::server::{NetcodeServer, Stopped};
#[cfg(feature = "server")]
use lightyear::prelude::{InterpolationTarget, LinkOf, NetworkTarget, Replicate};

#[cfg(feature = "server")]
use crate::movement::{DESTRUCTIBLE_TERRAIN_LAYER, INDESTRUCTIBLE_TERRAIN_LAYER};
use crate::protocol::{Fighter, NetworkEntityId, PlayerId};
#[cfg(feature = "server")]
use crate::timing::SimulationTick;
#[cfg(feature = "server")]
use crate::{
    gameplay::GameplaySet,
    movement::{
        ArenaWall, FIGHTER_LAYER, MovementTuning, PROJECTILE_LAYER, fighter_collision_layers,
        input_should_neutralize,
    },
    protocol::FighterInput,
};

/// The stable ID of the one code-authored fighter used by the combat sandbox.
pub const STANDARD_FIGHTER_DEFINITION: FighterDefinitionId = FighterDefinitionId(1);
/// The stable ID of the one code-authored weapon used by the combat sandbox.
pub const PULSE_SIDEARM_DEFINITION: WeaponDefinitionId = WeaponDefinitionId(1);
pub const SCATTER_CANNON_DEFINITION: WeaponDefinitionId = WeaponDefinitionId(2);
pub const ARC_LAUNCHER_DEFINITION: WeaponDefinitionId = WeaponDefinitionId(3);
pub const IMPACT_BLADE_DEFINITION: WeaponDefinitionId = WeaponDefinitionId(4);
/// Reserved team and entity identity for the neutral practice dummy.
pub const NEUTRAL_TEAM: TeamId = TeamId(u8::MAX);
pub const DUMMY_NETWORK_ENTITY: NetworkEntityId = NetworkEntityId(0);

/// Stable presentation colors make the replicated world-space ownership visible during
/// two-client smoke tests. These are presentation-only; gameplay never depends on color.
#[cfg(feature = "client")]
#[must_use]
pub fn fighter_color(player_id: PlayerId) -> Color {
    match player_id.0 {
        1 => Color::srgb(0.16, 0.62, 1.0),
        2 => Color::srgb(1.0, 0.42, 0.12),
        _ => {
            let hue_index = u16::try_from(player_id.0.wrapping_mul(137) % 360)
                .expect("palette hue fits in u16");
            let hue = f32::from(hue_index);
            Color::hsl(hue, 0.78, 0.56)
        }
    }
}

/// Return a brighter source color for a replicated projectile.
#[cfg(feature = "client")]
#[must_use]
pub fn projectile_color(player_id: PlayerId) -> Color {
    match player_id.0 {
        1 => Color::srgb(0.25, 0.86, 1.0),
        2 => Color::srgb(1.0, 0.78, 0.12),
        _ => fighter_color(player_id),
    }
}

/// Stable authored fighter definition identity.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Component, Reflect)]
pub struct FighterDefinitionId(pub u16);

/// Stable authored weapon definition identity.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Component, Reflect)]
pub struct WeaponDefinitionId(pub u16);

/// Authored fighter values. Runtime health and pose are components, not fields here.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct FighterDefinition {
    pub id: FighterDefinitionId,
    pub maximum_health: u16,
    pub movement_speed: f32,
    pub body_radius: f32,
    pub spawn_facing: f32,
    pub defeat_reset_delay_ticks: u64,
}

/// Authored pulse-sidearm values.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct WeaponDefinition {
    pub id: WeaponDefinitionId,
    pub direct_damage: u16,
    pub magazine_capacity: u8,
    pub fire_cooldown_ticks: u64,
    pub reload_duration_ticks: u64,
    pub projectile_speed: f32,
    pub projectile_radius: f32,
    pub maximum_range: f32,
    pub maximum_lifetime_ticks: u64,
    pub muzzle_offset: f32,
}

/// Immutable code-authored fighter catalog.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct FighterDefinitions {
    pub entries: Vec<FighterDefinition>,
}

impl Default for FighterDefinitions {
    fn default() -> Self {
        Self {
            entries: vec![FighterDefinition {
                id: STANDARD_FIGHTER_DEFINITION,
                maximum_health: 100,
                movement_speed: 320.0,
                body_radius: 24.0,
                spawn_facing: 0.0,
                defeat_reset_delay_ticks: 90,
            }],
        }
    }
}

impl FighterDefinitions {
    #[must_use]
    pub fn get(&self, id: FighterDefinitionId) -> Option<&FighterDefinition> {
        self.entries.iter().find(|definition| definition.id == id)
    }

    pub fn validate(&self, weapons: &WeaponDefinitions) -> Result<(), String> {
        if self.entries.is_empty() {
            return Err("fighter definition catalog is empty".to_string());
        }
        let mut ids = HashSet::new();
        for definition in &self.entries {
            if definition.id.0 == 0 || !ids.insert(definition.id) {
                return Err(format!(
                    "fighter definition ID {:?} is missing or duplicated",
                    definition.id
                ));
            }
            if definition.maximum_health == 0
                || !definition.movement_speed.is_finite()
                || definition.movement_speed <= 0.0
                || !definition.body_radius.is_finite()
                || definition.body_radius <= 0.0
                || !definition.spawn_facing.is_finite()
                || definition.defeat_reset_delay_ticks == 0
            {
                return Err(format!(
                    "fighter definition {:?} has invalid values",
                    definition.id
                ));
            }
            if weapons.get(PULSE_SIDEARM_DEFINITION).is_none() {
                return Err("standard fighter selects a missing pulse sidearm".to_string());
            }
        }
        Ok(())
    }
}

/// Immutable code-authored weapon catalog.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct WeaponDefinitions {
    pub entries: Vec<WeaponDefinition>,
}

impl Default for WeaponDefinitions {
    fn default() -> Self {
        Self {
            entries: vec![WeaponDefinition {
                id: PULSE_SIDEARM_DEFINITION,
                direct_damage: 25,
                magazine_capacity: 6,
                fire_cooldown_ticks: 12,
                reload_duration_ticks: 60,
                projectile_speed: 900.0,
                projectile_radius: 6.0,
                maximum_range: 900.0,
                maximum_lifetime_ticks: 60,
                muzzle_offset: 34.0,
            }],
        }
    }
}

impl WeaponDefinitions {
    #[must_use]
    pub fn get(&self, id: WeaponDefinitionId) -> Option<&WeaponDefinition> {
        self.entries.iter().find(|definition| definition.id == id)
    }

    pub fn validate(&self, fighter: &FighterDefinitions) -> Result<(), String> {
        if self.entries.is_empty() {
            return Err("weapon definition catalog is empty".to_string());
        }
        let mut ids = HashSet::new();
        for definition in &self.entries {
            if definition.id.0 == 0 || !ids.insert(definition.id) {
                return Err(format!(
                    "weapon definition ID {:?} is missing or duplicated",
                    definition.id
                ));
            }
            if definition.direct_damage == 0
                || definition.magazine_capacity == 0
                || definition.fire_cooldown_ticks == 0
                || definition.reload_duration_ticks == 0
                || definition.maximum_lifetime_ticks == 0
                || !definition.projectile_speed.is_finite()
                || definition.projectile_speed <= 0.0
                || !definition.projectile_radius.is_finite()
                || definition.projectile_radius <= 0.0
                || !definition.maximum_range.is_finite()
                || definition.maximum_range <= 0.0
                || !definition.muzzle_offset.is_finite()
                || definition.muzzle_offset <= 0.0
            {
                return Err(format!(
                    "weapon definition {:?} has invalid values",
                    definition.id
                ));
            }
            if definition.muzzle_offset
                < fighter.entries.first().map_or(0.0, |f| f.body_radius)
                    + definition.projectile_radius
            {
                return Err(format!(
                    "weapon definition {:?} starts inside its owner",
                    definition.id
                ));
            }
            let maximum_step = definition.projectile_speed / 60.0;
            if !maximum_step.is_finite()
                || maximum_step <= 0.0
                || maximum_step > definition.maximum_range
            {
                return Err(format!(
                    "weapon definition {:?} has an unrepresentable range step",
                    definition.id
                ));
            }
        }
        Ok(())
    }
}

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
    pub reset_at_tick: u64,
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

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageSource {
    PlayerWeapon {
        player_id: PlayerId,
        fighter_id: NetworkEntityId,
        weapon_definition_id: WeaponDefinitionId,
        shot_id: ShotId,
    },
    Environment {
        cause_id: u16,
    },
}

/// Cue variants used by deterministic/process evidence without serializing presentation payloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatCueKind {
    AttackAccepted,
    DeliveryImpact,
    LobLanded,
    MeleeContact,
    DamageApplied,
    EffectApplied,
    FighterDefeated,
    FighterReset,
    Muzzle,
    Impact,
    Damage,
    Defeat,
    Reset,
}

impl CombatCueKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AttackAccepted => "attack_accepted",
            Self::DeliveryImpact => "delivery_impact",
            Self::LobLanded => "lob_landed",
            Self::MeleeContact => "melee_contact",
            Self::DamageApplied => "damage_applied",
            Self::EffectApplied => "effect_applied",
            Self::FighterDefeated => "fighter_defeated",
            Self::FighterReset => "fighter_reset",
            Self::Muzzle => "muzzle",
            Self::Impact => "impact",
            Self::Damage => "damage",
            Self::Defeat => "defeat",
            Self::Reset => "reset",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "attack_accepted" => Some(Self::AttackAccepted),
            "delivery_impact" => Some(Self::DeliveryImpact),
            "lob_landed" => Some(Self::LobLanded),
            "melee_contact" => Some(Self::MeleeContact),
            "damage_applied" => Some(Self::DamageApplied),
            "effect_applied" => Some(Self::EffectApplied),
            "fighter_defeated" => Some(Self::FighterDefeated),
            "fighter_reset" => Some(Self::FighterReset),
            "muzzle" => Some(Self::Muzzle),
            "impact" => Some(Self::Impact),
            "damage" => Some(Self::Damage),
            "defeat" => Some(Self::Defeat),
            "reset" => Some(Self::Reset),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum CombatEffectCue {
    Knockback {
        velocity: WorldPoint,
        expires_at_tick: u64,
    },
    Slow {
        movement_multiplier_milli: u16,
        expires_at_tick: u64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CombatCueKey {
    pub kind: CombatCueKind,
    pub event_id: CombatEventId,
}

/// Ordered presentation facts. Durable values remain replicated components.
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum CombatCue {
    AttackAccepted {
        event_id: CombatEventId,
        tick: u64,
        attack_id: AttackId,
        source: NetworkEntityId,
        weapon_definition_id: WeaponDefinitionId,
        presentation_profile_id: WeaponPresentationProfileId,
    },
    DeliveryImpact {
        event_id: CombatEventId,
        tick: u64,
        attack_id: AttackId,
        delivery_index: u8,
        source: NetworkEntityId,
        weapon_definition_id: WeaponDefinitionId,
        presentation_profile_id: WeaponPresentationProfileId,
        target: Option<NetworkEntityId>,
        position: WorldPoint,
        normal: WorldPoint,
        distance_band: DistanceBand,
    },
    LobLanded {
        event_id: CombatEventId,
        tick: u64,
        attack_id: AttackId,
        delivery_index: u8,
        source: NetworkEntityId,
        weapon_definition_id: WeaponDefinitionId,
        presentation_profile_id: WeaponPresentationProfileId,
        position: WorldPoint,
    },
    MeleeContact {
        event_id: CombatEventId,
        tick: u64,
        attack_id: AttackId,
        delivery_index: u8,
        source: NetworkEntityId,
        weapon_definition_id: WeaponDefinitionId,
        presentation_profile_id: WeaponPresentationProfileId,
        target: NetworkEntityId,
        position: WorldPoint,
    },
    DamageApplied {
        event_id: CombatEventId,
        tick: u64,
        attack_id: AttackId,
        source: DamageSource,
        target: NetworkEntityId,
        amount: u16,
        health_after: u16,
        distance_band: DistanceBand,
        presentation_profile_id: WeaponPresentationProfileId,
    },
    EffectApplied {
        event_id: CombatEventId,
        tick: u64,
        attack_id: AttackId,
        source: DamageSource,
        target: NetworkEntityId,
        effect: CombatEffectCue,
        presentation_profile_id: WeaponPresentationProfileId,
    },
    FighterDefeated {
        event_id: CombatEventId,
        tick: u64,
        attack_id: AttackId,
        source: Option<DamageSource>,
        target: NetworkEntityId,
        presentation_profile_id: Option<WeaponPresentationProfileId>,
    },
    FighterReset {
        event_id: CombatEventId,
        tick: u64,
        target: NetworkEntityId,
        position: WorldPoint,
    },
    Muzzle {
        event_id: CombatEventId,
        tick: u64,
        source: NetworkEntityId,
        shot_id: ShotId,
        weapon_definition_id: WeaponDefinitionId,
        position: WorldPoint,
    },
    Impact {
        event_id: CombatEventId,
        tick: u64,
        source: NetworkEntityId,
        shot_id: ShotId,
        weapon_definition_id: WeaponDefinitionId,
        target: Option<NetworkEntityId>,
        position: WorldPoint,
        normal: WorldPoint,
        distance_band: DistanceBand,
    },
    Damage {
        event_id: CombatEventId,
        tick: u64,
        source: DamageSource,
        target: NetworkEntityId,
        amount: u16,
        health_after: u16,
        distance_band: DistanceBand,
    },
    Defeat {
        event_id: CombatEventId,
        tick: u64,
        source: Option<DamageSource>,
        target: NetworkEntityId,
    },
    Reset {
        event_id: CombatEventId,
        tick: u64,
        target: NetworkEntityId,
        position: WorldPoint,
    },
}

#[must_use]
pub fn combat_cue_key(cue: &CombatCue) -> CombatCueKey {
    let (kind, event_id) = match cue {
        CombatCue::AttackAccepted { event_id, .. } => (CombatCueKind::AttackAccepted, *event_id),
        CombatCue::DeliveryImpact { event_id, .. } => (CombatCueKind::DeliveryImpact, *event_id),
        CombatCue::LobLanded { event_id, .. } => (CombatCueKind::LobLanded, *event_id),
        CombatCue::MeleeContact { event_id, .. } => (CombatCueKind::MeleeContact, *event_id),
        CombatCue::DamageApplied { event_id, .. } => (CombatCueKind::DamageApplied, *event_id),
        CombatCue::EffectApplied { event_id, .. } => (CombatCueKind::EffectApplied, *event_id),
        CombatCue::FighterDefeated { event_id, .. } => (CombatCueKind::FighterDefeated, *event_id),
        CombatCue::FighterReset { event_id, .. } => (CombatCueKind::FighterReset, *event_id),
        CombatCue::Muzzle { event_id, .. } => (CombatCueKind::Muzzle, *event_id),
        CombatCue::Impact { event_id, .. } => (CombatCueKind::Impact, *event_id),
        CombatCue::Damage { event_id, .. } => (CombatCueKind::Damage, *event_id),
        CombatCue::Defeat { event_id, .. } => (CombatCueKind::Defeat, *event_id),
        CombatCue::Reset { event_id, .. } => (CombatCueKind::Reset, *event_id),
    };
    CombatCueKey { kind, event_id }
}

/// Encode a cue payload for the line-oriented process evidence file.
#[must_use]
pub fn encode_combat_cue(cue: &CombatCue) -> String {
    let bytes = postcard::to_allocvec(cue).expect("combat cue serialization should be infallible");
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use core::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

/// Decode a cue payload from the process evidence file.
#[must_use]
pub fn decode_combat_cue(encoded: &str) -> Option<CombatCue> {
    if !encoded.len().is_multiple_of(2) {
        return None;
    }
    let bytes = encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_value(pair[0])?;
            let low = hex_value(pair[1])?;
            Some((high << 4) | low)
        })
        .collect::<Option<Vec<_>>>()?;
    postcard::from_bytes(&bytes).ok()
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

/// Server counters retained across sandbox resets.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CombatTelemetry {
    pub accepted_shots: u64,
    pub hostile_fighter_hits: u64,
    pub applied_damage: u64,
    pub defeats: u64,
    pub dropped_cues: u64,
    pub dropped_records: u64,
    pub dropped_accepted_shot_timestamps: u64,
    pub close_hits: u64,
    pub mid_hits: u64,
    pub long_hits: u64,
    /// Wall-clock samples used only by the local impairment evidence harness. They are never
    /// used by gameplay or attribution.
    pub accepted_shot_timestamps: Vec<(ShotId, u128)>,
    /// Bounded authoritative cue payloads retained for deterministic/process evidence.
    pub cues: Vec<CombatCue>,
    /// Bounded diagnostic history; authoritative counters and cues are retained separately.
    pub records: Vec<CombatLogRecord>,
}

impl CombatTelemetry {
    pub fn record_cue(&mut self, cue: CombatCue) -> bool {
        if self.cues.len() < MAX_COMBAT_EVIDENCE_EVENTS {
            self.cues.push(cue);
            true
        } else {
            self.dropped_cues = self.dropped_cues.saturating_add(1);
            false
        }
    }

    pub fn record(&mut self, record: CombatLogRecord) {
        if self.records.len() < MAX_COMBAT_RECORDS {
            self.records.push(record);
        } else {
            self.dropped_records = self.dropped_records.saturating_add(1);
        }
    }
}

#[cfg(feature = "server")]
#[derive(Resource, Default, Debug)]
struct CombatSummaryLogged(bool);

#[cfg(feature = "server")]
#[derive(Resource, Clone, Copy, Debug, Default)]
struct CombatEvidenceMode {
    enabled: bool,
}

const MAX_COMBAT_EVIDENCE_EVENTS: usize = 512;
#[cfg(feature = "server")]
const COMBAT_CHECKPOINT_LATCH_TICKS: u16 = 600;
const MAX_COMBAT_RECORDS: usize = 512;
#[cfg(feature = "client")]
const MAX_COMBAT_SNAPSHOT_HISTORY: usize = 2048;

#[derive(Clone, Debug, PartialEq)]
pub enum CombatLogRecord {
    Shot {
        event_id: CombatEventId,
        tick: u64,
        shot_id: ShotId,
        source: NetworkEntityId,
        weapon: WeaponDefinitionId,
        muzzle_position: WorldPoint,
        ammo_after: u8,
    },
    Hit {
        tick: u64,
        event_id: CombatEventId,
        shot_id: ShotId,
        source: NetworkEntityId,
        target: Option<NetworkEntityId>,
        weapon: WeaponDefinitionId,
        position: WorldPoint,
        distance: f32,
        band: DistanceBand,
    },
    Damage {
        tick: u64,
        event_id: CombatEventId,
        source: DamageSource,
        target: NetworkEntityId,
        requested: u16,
        applied: u16,
        health_after: u16,
    },
    Defeat {
        tick: u64,
        event_id: CombatEventId,
        source: Option<DamageSource>,
        target: NetworkEntityId,
    },
    Reset {
        tick: u64,
        event_id: CombatEventId,
        target: NetworkEntityId,
        position: WorldPoint,
    },
}

#[must_use]
pub fn telemetry_cue_keys(records: &[CombatLogRecord]) -> Vec<CombatCueKey> {
    records
        .iter()
        .map(|record| match record {
            CombatLogRecord::Shot { event_id, .. } => CombatCueKey {
                kind: CombatCueKind::Muzzle,
                event_id: *event_id,
            },
            CombatLogRecord::Hit { event_id, .. } => CombatCueKey {
                kind: CombatCueKind::Impact,
                event_id: *event_id,
            },
            CombatLogRecord::Damage { event_id, .. } => CombatCueKey {
                kind: CombatCueKind::Damage,
                event_id: *event_id,
            },
            CombatLogRecord::Defeat { event_id, .. } => CombatCueKey {
                kind: CombatCueKind::Defeat,
                event_id: *event_id,
            },
            CombatLogRecord::Reset { event_id, .. } => CombatCueKey {
                kind: CombatCueKind::Reset,
                event_id: *event_id,
            },
        })
        .collect()
}

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct NextCombatIds {
    pub next_attack_id: u64,
    pub next_shot_id: u64,
    pub next_event_id: u64,
}

/// Return a process-comparable timestamp for the impairment evidence harness.
///
/// This value is deliberately outside the simulation contract: it is used only to compare
/// server acceptance and client cue receipt in local multi-process runs.
#[must_use]
pub fn unix_epoch_micros() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_micros())
}

impl Default for NextCombatIds {
    fn default() -> Self {
        Self {
            next_attack_id: 1,
            next_shot_id: 1,
            next_event_id: 1,
        }
    }
}

impl NextCombatIds {
    pub fn allocate_attack(&mut self) -> Option<AttackId> {
        let id = self.next_attack_id;
        self.next_attack_id = id.checked_add(1)?;
        Some(AttackId(id))
    }

    pub fn allocate_shot(&mut self) -> Option<ShotId> {
        let id = self.next_shot_id;
        self.next_shot_id = id.checked_add(1)?;
        Some(ShotId(id))
    }

    pub fn allocate_event(&mut self) -> Option<CombatEventId> {
        self.allocate_event_count(1)
    }

    pub fn allocate_event_pair(&mut self) -> Option<(CombatEventId, CombatEventId)> {
        let first = self.allocate_event_count(2)?;
        Some((first, CombatEventId(first.0 + 1)))
    }

    fn allocate_event_count(&mut self, count: u64) -> Option<CombatEventId> {
        let id = self.next_event_id;
        self.next_event_id = id.checked_add(count)?;
        Some(CombatEventId(id))
    }
}

#[must_use]
#[cfg(feature = "server")]
fn muzzle_position(position: Vec2, facing: f32, offset: f32) -> Vec2 {
    position + Vec2::from_angle(facing) * offset
}

#[must_use]
#[cfg(feature = "server")]
fn reset_is_due(current_tick: u64, reset_at_tick: u64) -> bool {
    current_tick >= reset_at_tick
}

/// Fixed-post authoritative combat ordering.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum CombatSet {
    ProjectileSweep,
    Damage,
    Lifecycle,
    TelemetryAndCues,
    Finalize,
}

#[must_use]
pub fn default_fighter_runtime(
    team_id: TeamId,
    fighters: &FighterDefinitions,
    weapons: &WeaponDefinitions,
) -> (
    FighterDefinitionId,
    SelectedBuild,
    TeamId,
    CurrentHealth,
    WeaponState,
) {
    let fighter = fighters
        .get(STANDARD_FIGHTER_DEFINITION)
        .expect("standard fighter definition exists");
    let build = SelectedBuild {
        primary_weapon: PULSE_SIDEARM_DEFINITION,
        source_preset_id: None,
        recipe_fingerprint: None,
    };
    let weapon = weapons
        .get(build.primary_weapon)
        .expect("standard weapon definition exists");
    (
        STANDARD_FIGHTER_DEFINITION,
        build,
        team_id,
        CurrentHealth(fighter.maximum_health),
        WeaponState {
            ammo: weapon.magazine_capacity,
            phase: WeaponPhase::Ready,
        },
    )
}

#[must_use]
pub fn sandbox_team(player_id: PlayerId) -> TeamId {
    TeamId(u8::try_from(player_id.0.saturating_sub(1) % 2).expect("team index fits in u8"))
}

/// Neutral entities are hostile to every team, including other neutral entities. Non-neutral
/// fighters are hostile only when their authored team IDs differ.
#[must_use]
pub fn teams_are_hostile(source: TeamId, target: TeamId) -> bool {
    source == NEUTRAL_TEAM || target == NEUTRAL_TEAM || source != target
}

/// M04's legacy cue/log shape is a compatibility adapter for the original single straight
/// direct-damage recipe. It is selected from the resolved recipe, never from a preset ID, and
/// does not participate in acceptance, collision, damage, or telemetry aggregation decisions.
#[cfg(feature = "server")]
fn legacy_compatibility_recipe(recipe: &WeaponRecipe) -> bool {
    matches!(recipe.firing, FiringPattern::Single)
        && matches!(recipe.delivery, DeliveryMethod::Straight { .. })
        && matches!(
            recipe.payload_bundles.as_slice(),
            [PayloadBundleDefinition {
                target: TargetSelection::Direct,
                effects
            }] if matches!(
                effects.as_slice(),
                [PayloadEffectDefinition::Damage {
                    falloff: DamageFalloff::None,
                    recipients: RecipientPolicy::Hostiles,
                    ..
                }]
            )
        )
}

#[cfg(feature = "server")]
pub struct ServerCombatPlugin;

#[cfg(feature = "server")]
impl Plugin for ServerCombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FighterDefinitions>()
            .init_resource::<WeaponDefinitions>()
            .init_resource::<MovementTuning>()
            .init_resource::<NextCombatIds>()
            .init_resource::<CombatTelemetry>()
            .init_resource::<WeaponTelemetry>()
            .init_resource::<ActiveAttackTrackers>()
            .init_resource::<CombatOutbox>()
            .init_resource::<CombatEvidenceSnapshots>()
            .init_resource::<CombatSummaryLogged>()
            .insert_resource(CombatEvidenceMode {
                enabled: env::var("BRAWLER_NETWORK_ASSERT_COMBAT").as_deref() == Ok("1"),
            })
            .add_message::<MeleeAttack>()
            .add_message::<PendingPayload>()
            .add_message::<PendingDelivery>()
            .add_systems(Startup, (validate_definitions, spawn_test_dummy).chain())
            .add_systems(
                FixedUpdate,
                (
                    reset_due_fighters.in_set(GameplaySet::Lifecycle),
                    expire_runtime_effects.in_set(GameplaySet::Lifecycle),
                    ApplyDeferred.after(GameplaySet::Lifecycle),
                    authoritative_composed_fire.in_set(GameplaySet::Fire),
                    ApplyDeferred.after(GameplaySet::Fire),
                ),
            )
            .add_systems(
                FixedPostUpdate,
                (
                    sweep_composed_projectiles
                        .after(avian2d::prelude::PhysicsSystems::StepSimulation)
                        .in_set(CombatSet::ProjectileSweep),
                    resolve_melee_attacks.in_set(CombatSet::Damage),
                    resolve_composed_payloads
                        .after(resolve_melee_attacks)
                        .in_set(CombatSet::Damage),
                    flush_completed_attack_telemetry.in_set(CombatSet::TelemetryAndCues),
                    send_combat_cues.in_set(CombatSet::TelemetryAndCues),
                    publish_authoritative_tick
                        .in_set(CombatSet::Finalize)
                        .before(crate::gameplay::advance_simulation_tick),
                    capture_server_combat_checkpoints
                        .in_set(CombatSet::Finalize)
                        .after(publish_authoritative_tick),
                    send_combat_evidence_checkpoints
                        .in_set(CombatSet::Finalize)
                        .after(capture_server_combat_checkpoints),
                ),
            )
            .add_systems(
                PreUpdate,
                cleanup_disconnected_projectiles
                    .after(lightyear::transport::plugin::TransportSystems::Receive),
            )
            .add_systems(Last, emit_combat_summary);
        let definition = *app
            .world()
            .resource::<FighterDefinitions>()
            .get(STANDARD_FIGHTER_DEFINITION)
            .expect("standard fighter definition exists");
        let mut tuning = app.world_mut().resource_mut::<MovementTuning>();
        tuning.speed = definition.movement_speed;
        tuning.radius = definition.body_radius;
        tuning.spawn_facing = definition.spawn_facing;
    }
}

fn validate_definitions(fighters: Res<FighterDefinitions>, weapons: Res<WeaponDefinitions>) {
    fighters
        .validate(&weapons)
        .expect("code-authored fighter definitions must be valid");
    weapons
        .validate(&fighters)
        .expect("code-authored weapon definitions must be valid");
}

#[cfg(feature = "server")]
fn spawn_test_dummy(
    mut commands: Commands,
    catalog: Res<WeaponCatalogResource>,
    fighters: Res<FighterDefinitions>,
    weapons: Res<WeaponDefinitions>,
) {
    if fighters.get(STANDARD_FIGHTER_DEFINITION).is_none()
        || weapons.get(PULSE_SIDEARM_DEFINITION).is_none()
    {
        return;
    }
    let Some(fighter) = fighters.get(STANDARD_FIGHTER_DEFINITION) else {
        return;
    };
    // Keep the dummy clear of the lower cover's collision body while leaving a deterministic
    // horizontal approach lane for the short-range process-combat profiles. The process harness
    // uses the open spawn row so every delivery family can reach the same target during its
    // bounded run; deterministic/unit harnesses retain the lower-row placement.
    let position = if env::var("BRAWLER_NETWORK_ASSERT_COMBAT").as_deref() == Ok("1") {
        Vec2::new(0.0, -300.0)
    } else {
        Vec2::new(0.0, -380.0)
    };
    let spawn_facing = fighter.spawn_facing;
    let body_radius = fighter.body_radius;
    let (fighter_definition, build, team, health, weapon) =
        default_fighter_runtime(NEUTRAL_TEAM, &fighters, &weapons);
    let resolved = catalog
        .0
        .resolve_preset(WeaponPresetId(PULSE_SIDEARM_DEFINITION.0), fighter)
        .expect("dummy pulse preset must resolve");
    let dummy = commands
        .spawn((
            Fighter,
            crate::movement::InputFreshness::default(),
            PlayerId(0),
            DUMMY_NETWORK_ENTITY,
            crate::protocol::PlaceholderState { spawn_slot: 255 },
            fighter_definition,
            build,
            team,
            health,
            weapon,
            Position::from_xy(position.x, position.y),
            Rotation::radians(spawn_facing),
            SpawnState {
                position,
                facing: spawn_facing,
            },
            LinearVelocity::default(),
            AngularVelocity::default(),
        ))
        .id();
    commands.entity(dummy).insert((
        SelectedBuild {
            primary_weapon: PULSE_SIDEARM_DEFINITION,
            source_preset_id: Some(WeaponPresetId(PULSE_SIDEARM_DEFINITION.0)),
            recipe_fingerprint: Some(resolved.recipe_fingerprint),
        },
        SelectedWeapon {
            source_preset_id: WeaponPresetId(PULSE_SIDEARM_DEFINITION.0),
            recipe_fingerprint: resolved.recipe_fingerprint,
        },
        resolved,
        AuthoritativeTick::default(),
        Collider::circle(body_radius),
        RigidBody::Kinematic,
        CustomPositionIntegration,
        fighter_collision_layers(),
        Replicate::to_clients(NetworkTarget::All),
        InterpolationTarget::to_clients(NetworkTarget::All),
        TestDummy,
    ));
}

/// Marks the reserved stationary hostile practice target.
#[cfg(feature = "server")]
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TestDummy;

#[cfg(feature = "server")]
fn reset_due_fighters(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    fighters: Res<FighterDefinitions>,
    weapons: Res<WeaponDefinitions>,
    mut telemetry: ResMut<CombatTelemetry>,
    mut ids: ResMut<NextCombatIds>,
    mut outbox: ResMut<CombatOutbox>,
    query: Query<(
        Entity,
        &NetworkEntityId,
        &FighterDefinitionId,
        &SelectedBuild,
        Option<&ResolvedWeapon>,
        &Defeated,
        &SpawnState,
    )>,
) {
    for (entity, network_id, fighter_id, build, resolved, defeated, spawn) in &query {
        if !reset_is_due(tick.0, defeated.reset_at_tick) {
            continue;
        }
        let Some(fighter) = fighters.get(*fighter_id) else {
            continue;
        };
        let (capacity, refill_ticks) = resolved
            .map_or_else(
                || {
                    weapons
                        .get(build.primary_weapon)
                        .map(|weapon| (weapon.magazine_capacity, weapon.reload_duration_ticks))
                },
                |weapon| {
                    Some((
                        weapon.recipe.economy.capacity(),
                        weapon.recipe.economy.refill_ticks(),
                    ))
                },
            )
            .unwrap_or((0, 0));
        if capacity == 0 || refill_ticks == 0 {
            continue;
        }
        let Some(event_id) = ids.allocate_event() else {
            continue;
        };
        let position = spawn.position;
        commands
            .entity(entity)
            .insert((
                CurrentHealth(fighter.maximum_health),
                WeaponState {
                    ammo: capacity,
                    phase: WeaponPhase::Ready,
                },
                Position::from_xy(position.x, position.y),
                Rotation::radians(spawn.facing),
                fighter_collision_layers(),
            ))
            .remove::<Defeated>()
            .remove::<ExternalMotion>()
            .remove::<KnockbackFeedback>()
            .insert(ActiveEffects::default());
        telemetry.record(CombatLogRecord::Reset {
            tick: tick.0,
            event_id,
            target: *network_id,
            position: WorldPoint::from(position),
        });
        info!(
            tick = tick.0,
            event_id = event_id.0,
            target = network_id.0,
            position = ?position,
            "authoritative fighter reset"
        );
        let cue = CombatCue::Reset {
            event_id,
            tick: tick.0,
            target: *network_id,
            position: WorldPoint::from(position),
        };
        telemetry.record_cue(cue.clone());
        outbox.0.push(cue);
        let _ = fighter;
    }
}

#[cfg(feature = "server")]
fn expire_runtime_effects(
    tick: Res<SimulationTick>,
    mut commands: Commands,
    mut fighters: Query<
        (
            Entity,
            &mut ActiveEffects,
            Option<&ExternalMotion>,
            Option<&KnockbackFeedback>,
            Option<&Defeated>,
        ),
        With<Fighter>,
    >,
) {
    for (entity, mut effects, external_motion, knockback, defeated) in &mut fighters {
        if defeated.is_some() {
            effects.slow = None;
            if external_motion.is_some() {
                commands
                    .entity(entity)
                    .remove::<ExternalMotion>()
                    .remove::<KnockbackFeedback>();
            }
            continue;
        }
        if effects
            .slow
            .is_some_and(|slow| tick.0 >= slow.expires_at_tick)
        {
            effects.slow = None;
        }
        if external_motion.is_some_and(|motion| tick.0 >= motion.expires_at_tick) {
            commands
                .entity(entity)
                .remove::<ExternalMotion>()
                .remove::<KnockbackFeedback>();
        } else if knockback.is_some_and(|feedback| tick.0 >= feedback.expires_at_tick) {
            commands.entity(entity).remove::<KnockbackFeedback>();
        }
    }
}

#[cfg(feature = "server")]
fn advance_composed_weapon_state(state: &mut WeaponState, recipe: &WeaponRecipe, tick: u64) {
    match state.phase {
        WeaponPhase::Cooldown { ready_at_tick } if tick >= ready_at_tick => {
            state.phase = WeaponPhase::Ready;
        }
        WeaponPhase::Reloading { ready_at_tick } if tick >= ready_at_tick => {
            state.ammo = recipe.economy.capacity();
            state.phase = WeaponPhase::Ready;
        }
        _ => {}
    }
}

#[cfg(feature = "server")]
fn authoritative_composed_fire(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    arena: Res<crate::movement::GreyboxArenaDefinition>,
    spatial_query: avian2d::prelude::SpatialQuery,
    disconnected: Query<Entity, (With<LinkOf>, With<lightyear::prelude::Disconnected>)>,
    mut ids: ResMut<NextCombatIds>,
    mut telemetry: ResMut<WeaponTelemetry>,
    mut legacy_telemetry: ResMut<CombatTelemetry>,
    evidence: Res<CombatEvidenceMode>,
    mut trackers: ResMut<ActiveAttackTrackers>,
    mut outbox: ResMut<CombatOutbox>,
    mut melee: MessageWriter<MeleeAttack>,
    query: Query<
        (
            Entity,
            &Position,
            &Rotation,
            &SelectedBuild,
            &ResolvedWeapon,
            &TeamId,
            &PlayerId,
            &NetworkEntityId,
            Option<&lightyear::prelude::ControlledBy>,
            &crate::movement::InputFreshness,
            &mut WeaponState,
            Option<&ActionState<FighterInput>>,
            Option<&Defeated>,
            Option<&AwaitingPostSelectionInput>,
        ),
        With<Fighter>,
    >,
) {
    let disconnected: HashSet<_> = disconnected.iter().collect();
    for (
        entity,
        position,
        rotation,
        build,
        resolved,
        team,
        player_id,
        network_id,
        controlled_by,
        freshness,
        mut state,
        action,
        defeated,
        activation_barrier,
    ) in query
    {
        if controlled_by.is_some_and(|controlled| disconnected.contains(&controlled.owner)) {
            continue;
        }
        if defeated.is_some() || activation_barrier.is_some() {
            continue;
        }
        let recipe = &resolved.recipe;
        advance_composed_weapon_state(&mut state, recipe, tick.0);
        let input = action.map_or(FighterInput::default(), |value| value.0);
        let held = !input_should_neutralize(tick.0, freshness.last_fresh_tick, 12)
            && input.is_valid()
            && input.gameplay_buttons & FighterInput::PRIMARY_FIRE != 0;
        if !held || !matches!(state.phase, WeaponPhase::Ready) {
            if held && state.ammo == 0 && matches!(state.phase, WeaponPhase::Ready) {
                state.phase = WeaponPhase::Reloading {
                    ready_at_tick: tick.0.saturating_add(recipe.economy.refill_ticks()),
                };
            }
            continue;
        }
        if state.ammo == 0 {
            state.phase = WeaponPhase::Reloading {
                ready_at_tick: tick.0.saturating_add(recipe.economy.refill_ticks()),
            };
            continue;
        }
        let origin = position.0;
        let facing = rotation.as_radians();
        let lob_landing = match recipe.delivery {
            DeliveryMethod::Lobbed {
                distance,
                landing_clearance_radius,
                ..
            } => {
                let desired = origin + Vec2::from_angle(facing) * distance;
                let bounded = desired.clamp(
                    arena.min + Vec2::splat(landing_clearance_radius),
                    arena.max - Vec2::splat(landing_clearance_radius),
                );
                let terrain_filter = avian2d::prelude::SpatialQueryFilter::from_mask(
                    INDESTRUCTIBLE_TERRAIN_LAYER | DESTRUCTIBLE_TERRAIN_LAYER,
                );
                delivery::repaired_landing_point(
                    origin,
                    bounded,
                    landing_clearance_radius,
                    |candidate| {
                        spatial_query
                            .shape_intersections(
                                &Collider::circle(landing_clearance_radius),
                                candidate,
                                0.0,
                                &terrain_filter,
                            )
                            .is_empty()
                    },
                )
            }
            _ => None,
        };
        if matches!(recipe.delivery, DeliveryMethod::Lobbed { .. }) && lob_landing.is_none() {
            continue;
        }
        let legacy_compatibility = legacy_compatibility_recipe(recipe);
        let blocked_deliveries = match recipe.delivery {
            DeliveryMethod::Straight {
                radius,
                muzzle_offset,
                ..
            } => spread_angles(
                facing,
                match recipe.firing {
                    FiringPattern::Single => 1,
                    FiringPattern::Spread { delivery_count, .. } => delivery_count,
                },
                match recipe.firing {
                    FiringPattern::Single => 0.0,
                    FiringPattern::Spread {
                        total_angle_degrees,
                        ..
                    } => total_angle_degrees,
                },
            )
            .into_iter()
            .enumerate()
            .filter_map(|(index, angle)| {
                let muzzle = muzzle_position(origin, angle, muzzle_offset);
                terrain_muzzle_contact(origin, muzzle, radius, &spatial_query)
                    .map(|(point, normal)| (u8::try_from(index).unwrap_or(u8::MAX), point, normal))
            })
            .collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        let per_blocked_delivery_events = if legacy_compatibility { 2 } else { 1 };
        let event_count = 1
            + usize::from(legacy_compatibility)
            + blocked_deliveries.len() * per_blocked_delivery_events;
        let Some((attack_id, reserved_events)) =
            server::reserve_attack_and_events(&mut ids, event_count)
        else {
            continue;
        };
        let event_id = reserved_events[0];
        let legacy_muzzle_event = if legacy_compatibility {
            Some(reserved_events[1])
        } else {
            None
        };
        let mut blocked_event_cursor = 1 + usize::from(legacy_compatibility);
        state.ammo = state.ammo.saturating_sub(1);
        state.phase = if state.ammo == 0 {
            WeaponPhase::Reloading {
                ready_at_tick: tick.0.saturating_add(recipe.economy.refill_ticks()),
            }
        } else {
            WeaponPhase::Cooldown {
                ready_at_tick: tick.0.saturating_add(recipe.fire_cooldown_ticks),
            }
        };
        let preset_id = resolved.source_preset_id;
        let source = AttackSource {
            attack_id,
            player_id: *player_id,
            owner_network_entity_id: *network_id,
            team_id: *team,
            recipe_fingerprint: resolved.recipe_fingerprint,
            presentation_profile_id: resolved.presentation_profile_id,
            legacy_compatibility,
            source_preset_id: preset_id,
            origin: WorldPoint::from(origin),
            facing,
        };
        let weapon_id = build.primary_weapon;
        let source_component = ProjectileSource {
            shot_id: ShotId(attack_id.0),
            player_id: *player_id,
            owner_network_entity_id: *network_id,
            team_id: *team,
            weapon_definition_id: weapon_id,
        };
        let mut emitted_deliveries = 0_u64;
        match recipe.delivery {
            DeliveryMethod::Straight {
                speed,
                radius,
                range,
                lifetime_ticks,
                muzzle_offset,
            } => {
                let angles = spread_angles(
                    facing,
                    match recipe.firing {
                        FiringPattern::Single => 1,
                        FiringPattern::Spread { delivery_count, .. } => delivery_count,
                    },
                    match recipe.firing {
                        FiringPattern::Single => 0.0,
                        FiringPattern::Spread {
                            total_angle_degrees,
                            ..
                        } => total_angle_degrees,
                    },
                );
                for (delivery_index, angle) in angles.into_iter().enumerate() {
                    let delivery_index = u8::try_from(delivery_index).unwrap_or(u8::MAX);
                    let muzzle = muzzle_position(origin, angle, muzzle_offset);
                    if let Some((point, normal)) = blocked_deliveries
                        .iter()
                        .find(|(blocked_index, _, _)| *blocked_index == delivery_index)
                        .map(|(_, point, normal)| (*point, *normal))
                    {
                        let impact_event_id = reserved_events[blocked_event_cursor];
                        blocked_event_cursor += 1;
                        let impact_cue = CombatCue::DeliveryImpact {
                            event_id: impact_event_id,
                            tick: tick.0,
                            attack_id,
                            delivery_index,
                            source: *network_id,
                            weapon_definition_id: weapon_id,
                            presentation_profile_id: resolved.presentation_profile_id,
                            target: None,
                            position: WorldPoint::from(point),
                            normal: WorldPoint::from(normal),
                            distance_band: distance_band(origin.distance(point)),
                        };
                        legacy_telemetry.record_cue(impact_cue.clone());
                        outbox.0.push(impact_cue);
                        if legacy_compatibility {
                            let legacy_event = reserved_events[blocked_event_cursor];
                            blocked_event_cursor += 1;
                            let legacy_cue = CombatCue::Impact {
                                event_id: legacy_event,
                                tick: tick.0,
                                source: *network_id,
                                shot_id: ShotId(attack_id.0),
                                weapon_definition_id: weapon_id,
                                target: None,
                                position: WorldPoint::from(point),
                                normal: WorldPoint::from(normal),
                                distance_band: distance_band(origin.distance(point)),
                            };
                            legacy_telemetry.record_cue(legacy_cue.clone());
                            legacy_telemetry.record(CombatLogRecord::Hit {
                                tick: tick.0,
                                event_id: legacy_event,
                                shot_id: ShotId(attack_id.0),
                                source: *network_id,
                                target: None,
                                weapon: weapon_id,
                                position: WorldPoint::from(point),
                                distance: origin.distance(point),
                                band: distance_band(origin.distance(point)),
                            });
                            outbox.0.push(legacy_cue);
                        }
                        emitted_deliveries = emitted_deliveries.saturating_add(1);
                        continue;
                    }
                    commands.spawn((
                        Projectile,
                        source_component,
                        ReplicatedAttackSource { attack: source },
                        AttackDelivery {
                            attack_id,
                            delivery_index,
                        },
                        ProjectileDeadline {
                            expires_at_tick: tick.0.saturating_add(lifetime_ticks),
                        },
                        StraightFlight {
                            origin: WorldPoint::from(muzzle),
                            facing: angle,
                            speed,
                            maximum_range: range,
                            launched_at_tick: tick.0,
                        },
                        ComposedProjectileRuntime {
                            owner_entity: entity,
                            source,
                            delivery_index,
                            velocity: Vec2::from_angle(angle) * speed,
                            travelled: 0.0,
                            expires_at_tick: tick.0.saturating_add(lifetime_ticks),
                            maximum_range: range,
                            radius,
                            landing: None,
                            recipe: recipe.clone(),
                        },
                        Position::from_xy(muzzle.x, muzzle.y),
                        Rotation::radians(angle),
                        Collider::circle(radius),
                        CollisionLayers::new(
                            PROJECTILE_LAYER,
                            FIGHTER_LAYER
                                | INDESTRUCTIBLE_TERRAIN_LAYER
                                | DESTRUCTIBLE_TERRAIN_LAYER,
                        ),
                        Replicate::to_clients(NetworkTarget::All),
                        InterpolationTarget::to_clients(NetworkTarget::All),
                    ));
                    emitted_deliveries = emitted_deliveries.saturating_add(1);
                }
            }
            DeliveryMethod::Lobbed {
                distance,
                flight_ticks,
                visual_arc_height,
                landing_clearance_radius: _,
                muzzle_offset,
            } => {
                let landing = lob_landing.expect("validated lob landing must exist");
                let launch = muzzle_position(origin, facing, muzzle_offset);
                commands.spawn((
                    Projectile,
                    source_component,
                    ReplicatedAttackSource { attack: source },
                    AttackDelivery {
                        attack_id,
                        delivery_index: 0,
                    },
                    ProjectileDeadline {
                        expires_at_tick: tick.0.saturating_add(flight_ticks),
                    },
                    LobbedFlight {
                        launch: WorldPoint::from(launch),
                        landing: WorldPoint::from(landing),
                        launched_at_tick: tick.0,
                        lands_at_tick: tick.0.saturating_add(flight_ticks),
                        visual_arc_height,
                    },
                    ComposedProjectileRuntime {
                        owner_entity: entity,
                        source,
                        delivery_index: 0,
                        velocity: Vec2::ZERO,
                        travelled: 0.0,
                        expires_at_tick: tick.0.saturating_add(flight_ticks),
                        maximum_range: distance,
                        radius: 0.0,
                        landing: Some(landing),
                        recipe: recipe.clone(),
                    },
                    Position::from_xy(launch.x, launch.y),
                    Rotation::radians(facing),
                    Replicate::to_clients(NetworkTarget::All),
                    InterpolationTarget::to_clients(NetworkTarget::All),
                ));
                emitted_deliveries = 1;
            }
            DeliveryMethod::MeleeArc { .. } => {
                melee.write(MeleeAttack {
                    source,
                    origin,
                    facing,
                    tick: tick.0,
                    recipe: recipe.clone(),
                });
                emitted_deliveries = 1;
            }
        }
        let preset_id = preset_id.unwrap_or(WeaponPresetId(weapon_id.0));
        telemetry.record_emitted_deliveries(
            preset_id,
            resolved.recipe_fingerprint,
            emitted_deliveries,
        );
        if emitted_deliveries > 0 {
            if trackers.active.len() < server::MAX_ACTIVE_ATTACK_TRACKERS {
                trackers.active.insert(
                    attack_id,
                    ActiveAttackTracker {
                        source,
                        expected_deliveries: u8::try_from(emitted_deliveries).unwrap_or(u8::MAX),
                        resolved_deliveries: 0,
                        had_hostile_contact: false,
                    },
                );
            } else {
                telemetry.tracker_drops = telemetry.tracker_drops.saturating_add(1);
            }
        }
        for _ in 0..blocked_deliveries.len() {
            finish_attack_delivery(&mut trackers, attack_id);
        }
        telemetry.record_accepted_attack(preset_id, resolved.recipe_fingerprint);
        legacy_telemetry.accepted_shots = legacy_telemetry.accepted_shots.saturating_add(1);
        let accepted_cue = CombatCue::AttackAccepted {
            event_id,
            tick: tick.0,
            attack_id,
            source: *network_id,
            weapon_definition_id: weapon_id,
            presentation_profile_id: resolved.presentation_profile_id,
        };
        legacy_telemetry.record_cue(accepted_cue.clone());
        outbox.0.push(accepted_cue);
        if evidence.enabled {
            if legacy_telemetry.accepted_shot_timestamps.len() < MAX_COMBAT_EVIDENCE_EVENTS {
                legacy_telemetry
                    .accepted_shot_timestamps
                    .push((ShotId(attack_id.0), unix_epoch_micros()));
            } else {
                legacy_telemetry.dropped_accepted_shot_timestamps = legacy_telemetry
                    .dropped_accepted_shot_timestamps
                    .saturating_add(1);
            }
        }
        if let Some(muzzle_event) = legacy_muzzle_event {
            let muzzle = muzzle_position(
                origin,
                facing,
                match recipe.delivery {
                    DeliveryMethod::Straight { muzzle_offset, .. }
                    | DeliveryMethod::Lobbed { muzzle_offset, .. } => muzzle_offset,
                    DeliveryMethod::MeleeArc { .. } => 0.0,
                },
            );
            legacy_telemetry.record(CombatLogRecord::Shot {
                event_id: muzzle_event,
                tick: tick.0,
                shot_id: ShotId(attack_id.0),
                source: *network_id,
                weapon: weapon_id,
                muzzle_position: WorldPoint::from(muzzle),
                ammo_after: state.ammo,
            });
            let muzzle_cue = CombatCue::Muzzle {
                event_id: muzzle_event,
                tick: tick.0,
                source: *network_id,
                shot_id: ShotId(attack_id.0),
                weapon_definition_id: weapon_id,
                position: WorldPoint::from(muzzle),
            };
            legacy_telemetry.record_cue(muzzle_cue.clone());
            outbox.0.push(muzzle_cue);
        }
        telemetry.record(WeaponTelemetryRecord {
            tick: tick.0,
            event_id,
            attack_id,
            preset_id,
            recipe_fingerprint: resolved.recipe_fingerprint,
            delivery_index: None,
            source: *network_id,
            target: None,
            position: WorldPoint::from(origin),
            requested_value: 0,
            applied_value: 0,
            engagement_distance: 0.0,
            delivery_travel: 0.0,
            hostile_contact: false,
            effect: None,
            resulting_health: None,
            resulting_effects: None,
            resulting_motion: None,
            outcome: WeaponTelemetryOutcome::AttackAccepted,
        });
    }
}

#[cfg(feature = "server")]
fn payload_can_affect_target(
    bundle: &PayloadBundleDefinition,
    source: AttackSource,
    target_team: TeamId,
    target_network_id: NetworkEntityId,
) -> bool {
    if target_network_id == source.owner_network_entity_id {
        return bundle.effects.iter().any(|effect| {
            matches!(
                effect,
                PayloadEffectDefinition::Damage {
                    recipients: RecipientPolicy::HostilesAndOwner { .. },
                    ..
                } | PayloadEffectDefinition::Knockback {
                    recipients: RecipientPolicy::HostilesAndOwner { .. },
                    ..
                }
            )
        });
    }
    teams_are_hostile(source.team_id, target_team)
}

#[cfg(feature = "server")]
fn area_line_of_sight_clear(
    origin: Vec2,
    target: Vec2,
    spatial_query: &avian2d::prelude::SpatialQuery,
) -> bool {
    let delta = target - origin;
    let distance = delta.length();
    let Some(direction) = Dir2::new(delta.normalize_or_zero()).ok() else {
        return true;
    };
    let filter = avian2d::prelude::SpatialQueryFilter::from_mask(
        INDESTRUCTIBLE_TERRAIN_LAYER | DESTRUCTIBLE_TERRAIN_LAYER,
    );
    spatial_query
        .cast_ray(origin, direction, distance.max(0.0), true, &filter)
        .is_none()
}

#[cfg(feature = "server")]
fn terrain_muzzle_contact(
    origin: Vec2,
    muzzle: Vec2,
    radius: f32,
    spatial_query: &avian2d::prelude::SpatialQuery,
) -> Option<(Vec2, Vec2)> {
    let delta = muzzle - origin;
    let distance = delta.length();
    let direction = Dir2::new(delta.normalize_or_zero()).ok()?;
    let filter = avian2d::prelude::SpatialQueryFilter::from_mask(
        INDESTRUCTIBLE_TERRAIN_LAYER | DESTRUCTIBLE_TERRAIN_LAYER,
    );
    spatial_query
        .cast_shape_predicate(
            &Collider::circle(radius),
            origin,
            0.0,
            direction,
            &avian2d::prelude::ShapeCastConfig::from_max_distance(distance),
            &filter,
            &|_| true,
        )
        .map(|hit| (hit.point2, hit.normal1))
}

#[cfg(feature = "server")]
fn queue_area_payloads(
    landing: Vec2,
    source: AttackSource,
    delivery_index: u8,
    recipe: &WeaponRecipe,
    fighters: &Query<
        (
            Entity,
            &Position,
            &TeamId,
            &NetworkEntityId,
            Option<&Defeated>,
            Option<&lightyear::prelude::ControlledBy>,
        ),
        With<Fighter>,
    >,
    disconnected: &HashSet<Entity>,
    spatial_query: &avian2d::prelude::SpatialQuery,
    pending: &mut MessageWriter<PendingPayload>,
) -> usize {
    let mut queued = 0;
    let fighter_filter = avian2d::prelude::SpatialQueryFilter::from_mask(FIGHTER_LAYER);
    for (bundle_index, bundle) in recipe
        .payload_bundles
        .iter()
        .enumerate()
        .filter(|(_, bundle)| matches!(bundle.target, TargetSelection::Area { .. }))
    {
        let TargetSelection::Area {
            radius,
            terrain_occlusion,
            max_targets,
        } = bundle.target
        else {
            continue;
        };
        let candidate_entities = spatial_query.shape_intersections(
            &Collider::circle(radius),
            landing,
            0.0,
            &fighter_filter,
        );
        let mut candidates: Vec<_> = candidate_entities
            .into_iter()
            .filter_map(|entity| fighters.get(entity).ok().map(|data| (entity, data)))
            .collect();
        candidates.sort_by_key(|(_, (_, _, _, network_id, _, _))| network_id.0);
        let mut collected = 0_u8;
        for (target, (_, position, team, network_id, defeated, controlled)) in candidates {
            if collected >= max_targets {
                break;
            }
            if defeated.is_some()
                || controlled.is_some_and(|controlled| disconnected.contains(&controlled.owner))
                || (terrain_occlusion
                    && !area_line_of_sight_clear(landing, position.0, spatial_query))
                || !payload_can_affect_target(bundle, source, *team, *network_id)
            {
                continue;
            }
            pending.write(PendingPayload {
                source,
                delivery_index,
                bundle_index: u8::try_from(bundle_index).unwrap_or(u8::MAX),
                target,
                target_network_id: *network_id,
                position: landing,
                engagement_distance: source.origin.as_vec2().distance(position.0),
                delivery_travel: lob_launch_point(source, recipe).distance(landing),
                contact_fraction: 1.0,
                bundle: bundle.clone(),
            });
            queued += 1;
            collected = collected.saturating_add(1);
        }
    }
    queued
}

#[cfg(feature = "server")]
fn lob_launch_point(source: AttackSource, recipe: &WeaponRecipe) -> Vec2 {
    let muzzle_offset = match recipe.delivery {
        DeliveryMethod::Lobbed { muzzle_offset, .. } => muzzle_offset,
        _ => 0.0,
    };
    muzzle_position(source.origin.as_vec2(), source.facing, muzzle_offset)
}

#[cfg(feature = "server")]
fn record_delivery_termination(
    ids: &mut NextCombatIds,
    telemetry: &mut WeaponTelemetry,
    tick: u64,
    runtime: &ComposedProjectileRuntime,
    position: Vec2,
    outcome: WeaponTelemetryOutcome,
) {
    let Some(event_id) = ids.allocate_event() else {
        return;
    };
    telemetry.record(WeaponTelemetryRecord {
        tick,
        event_id,
        attack_id: runtime.source.attack_id,
        preset_id: runtime.source.source_preset_id.unwrap_or(WeaponPresetId(0)),
        recipe_fingerprint: runtime.source.recipe_fingerprint,
        delivery_index: Some(runtime.delivery_index),
        source: runtime.source.owner_network_entity_id,
        target: None,
        position: WorldPoint::from(position),
        requested_value: 0,
        applied_value: 0,
        engagement_distance: 0.0,
        delivery_travel: runtime.travelled,
        hostile_contact: false,
        effect: None,
        resulting_health: None,
        resulting_effects: None,
        resulting_motion: None,
        outcome,
    });
}

#[cfg(feature = "server")]
fn sweep_composed_projectiles(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    mut ids: ResMut<NextCombatIds>,
    mut trackers: ResMut<ActiveAttackTrackers>,
    mut telemetry: ResMut<WeaponTelemetry>,
    mut pending: MessageWriter<PendingPayload>,
    mut deliveries: MessageWriter<PendingDelivery>,
    mut projectiles: Query<(
        Entity,
        &Position,
        &mut ComposedProjectileRuntime,
        Option<&LobbedFlight>,
    )>,
    fighters: Query<
        (
            Entity,
            &Position,
            &TeamId,
            &NetworkEntityId,
            Option<&Defeated>,
            Option<&lightyear::prelude::ControlledBy>,
        ),
        With<Fighter>,
    >,
    disconnected: Query<Entity, (With<LinkOf>, With<lightyear::prelude::Disconnected>)>,
    walls: Query<Entity, With<ArenaWall>>,
    spatial_query: avian2d::prelude::SpatialQuery,
) {
    let disconnected: HashSet<_> = disconnected.iter().collect();
    let fighter_lookup: HashMap<_, _> = fighters
        .iter()
        .map(
            |(entity, position, team, network_id, defeated, controlled)| {
                (
                    entity,
                    (
                        position.0,
                        *team,
                        *network_id,
                        defeated.is_some(),
                        controlled
                            .is_some_and(|controlled| disconnected.contains(&controlled.owner)),
                    ),
                )
            },
        )
        .collect();
    let wall_entities: HashSet<_> = walls.iter().collect();
    let mut ordered: Vec<_> = projectiles.iter_mut().collect();
    ordered.sort_by_key(|(_, _, runtime, lob)| {
        (
            runtime.source.attack_id.0,
            runtime.delivery_index,
            lob.is_some(),
        )
    });
    for (entity, position, mut runtime, lob) in ordered {
        let Some((_, _, _, _, owner_disconnected)) = fighter_lookup.get(&runtime.owner_entity)
        else {
            record_delivery_termination(
                &mut ids,
                &mut telemetry,
                tick.0,
                &runtime,
                position.0,
                WeaponTelemetryOutcome::DeliveryCancelled,
            );
            commands.entity(entity).despawn();
            finish_attack_delivery(&mut trackers, runtime.source.attack_id);
            continue;
        };
        if *owner_disconnected {
            record_delivery_termination(
                &mut ids,
                &mut telemetry,
                tick.0,
                &runtime,
                position.0,
                WeaponTelemetryOutcome::DeliveryCancelled,
            );
            commands.entity(entity).despawn();
            finish_attack_delivery(&mut trackers, runtime.source.attack_id);
            continue;
        }
        if let Some(lob) = lob {
            if tick.0 < lob.lands_at_tick {
                let progress = (tick.0.saturating_sub(lob.launched_at_tick) as f32)
                    / (lob
                        .lands_at_tick
                        .saturating_sub(lob.launched_at_tick)
                        .max(1) as f32);
                let launch = lob.launch.as_vec2();
                let landing = lob.landing.as_vec2();
                commands
                    .entity(entity)
                    .insert(Position(launch.lerp(landing, progress.clamp(0.0, 1.0))));
                continue;
            }
            let landing = lob.landing.as_vec2();
            let _queued_payloads = queue_area_payloads(
                landing,
                runtime.source,
                runtime.delivery_index,
                &runtime.recipe,
                &fighters,
                &disconnected,
                &spatial_query,
                &mut pending,
            );
            deliveries.write(PendingDelivery {
                entity: Some(entity),
                source: runtime.source,
                delivery_index: runtime.delivery_index,
                tick: tick.0,
                engagement_distance: 0.0,
                delivery_travel: lob_launch_point(runtime.source, &runtime.recipe)
                    .distance(landing),
                kind: PendingDeliveryKind::LobLanded {
                    position: WorldPoint::from(landing),
                },
            });
            continue;
        }
        if tick.0 >= runtime.expires_at_tick || runtime.travelled >= runtime.maximum_range {
            record_delivery_termination(
                &mut ids,
                &mut telemetry,
                tick.0,
                &runtime,
                position.0,
                WeaponTelemetryOutcome::DeliveryExpired,
            );
            commands.entity(entity).despawn();
            finish_attack_delivery(&mut trackers, runtime.source.attack_id);
            continue;
        }
        let step = (runtime.velocity.length() / 60.0)
            .min((runtime.maximum_range - runtime.travelled).max(0.0));
        let Some(direction) = Dir2::new(runtime.velocity.normalize_or_zero()).ok() else {
            record_delivery_termination(
                &mut ids,
                &mut telemetry,
                tick.0,
                &runtime,
                position.0,
                WeaponTelemetryOutcome::DeliveryCancelled,
            );
            commands.entity(entity).despawn();
            finish_attack_delivery(&mut trackers, runtime.source.attack_id);
            continue;
        };
        let filter = avian2d::prelude::SpatialQueryFilter::from_mask(
            FIGHTER_LAYER | INDESTRUCTIBLE_TERRAIN_LAYER | DESTRUCTIBLE_TERRAIN_LAYER,
        )
        .with_excluded_entities([entity, runtime.owner_entity]);
        let hit = spatial_query.cast_shape_predicate(
            &Collider::circle(runtime.radius),
            position.0,
            0.0,
            direction,
            &avian2d::prelude::ShapeCastConfig::from_max_distance(step),
            &filter,
            &|candidate| {
                fighter_lookup.get(&candidate).map_or_else(
                    || wall_entities.contains(&candidate),
                    |(_, team, _, defeated, owner_disconnected)| {
                        teams_are_hostile(runtime.source.team_id, *team)
                            && !defeated
                            && !owner_disconnected
                    },
                )
            },
        );
        let Some(hit) = hit else {
            runtime.travelled += step;
            commands
                .entity(entity)
                .insert(Position(position.0 + direction.as_vec2() * step));
            continue;
        };
        runtime.travelled += hit.distance.clamp(0.0, step);
        let target = fighter_lookup.get(&hit.entity).copied();
        if let Some((target_position, target_team, target_network_id, defeated, _)) = target
            && !defeated
            && teams_are_hostile(runtime.source.team_id, target_team)
        {
            for (bundle_index, bundle) in
                runtime
                    .recipe
                    .payload_bundles
                    .iter()
                    .enumerate()
                    .filter(|(_, bundle)| {
                        matches!(bundle.target, TargetSelection::Direct)
                            && payload_can_affect_target(
                                bundle,
                                runtime.source,
                                target_team,
                                target_network_id,
                            )
                    })
            {
                pending.write(PendingPayload {
                    source: runtime.source,
                    delivery_index: runtime.delivery_index,
                    bundle_index: u8::try_from(bundle_index).unwrap_or(u8::MAX),
                    target: hit.entity,
                    target_network_id,
                    position: hit.point2,
                    engagement_distance: runtime.source.origin.as_vec2().distance(target_position),
                    delivery_travel: runtime.travelled,
                    contact_fraction: (hit.distance / step.max(f32::EPSILON)).clamp(0.0, 1.0),
                    bundle: bundle.clone(),
                });
            }
        }
        deliveries.write(PendingDelivery {
            entity: Some(entity),
            source: runtime.source,
            delivery_index: runtime.delivery_index,
            tick: tick.0,
            engagement_distance: target.map_or(0.0, |(position, ..)| {
                runtime.source.origin.as_vec2().distance(position)
            }),
            delivery_travel: runtime.travelled,
            kind: PendingDeliveryKind::StraightImpact {
                target: target.map(|(_, _, network_id, _, _)| network_id),
                position: WorldPoint::from(hit.point2),
                normal: WorldPoint::from(hit.normal1),
                distance_band: distance_band(runtime.travelled),
            },
        });
    }
}

#[cfg(feature = "server")]
fn resolve_melee_attacks(
    mut attacks: MessageReader<MeleeAttack>,
    mut pending: MessageWriter<PendingPayload>,
    mut deliveries: MessageWriter<PendingDelivery>,
    mut trackers: ResMut<ActiveAttackTrackers>,
    disconnected: Query<Entity, (With<LinkOf>, With<lightyear::prelude::Disconnected>)>,
    fighters: Query<
        (
            Entity,
            &Position,
            &TeamId,
            &NetworkEntityId,
            Option<&Defeated>,
            Option<&lightyear::prelude::ControlledBy>,
        ),
        With<Fighter>,
    >,
    spatial_query: avian2d::prelude::SpatialQuery,
    tuning: Res<MovementTuning>,
) {
    let disconnected: HashSet<_> = disconnected.iter().collect();
    for attack in attacks.read() {
        let owner_connected = fighters.iter().any(|(_, _, _, network_id, _, controlled)| {
            *network_id == attack.source.owner_network_entity_id
                && controlled.is_none_or(|controlled| !disconnected.contains(&controlled.owner))
        });
        if !owner_connected {
            finish_attack_delivery(&mut trackers, attack.source.attack_id);
            continue;
        }
        let Some((reach, angle)) = (match attack.recipe.delivery {
            DeliveryMethod::MeleeArc {
                reach,
                angle_degrees,
            } => Some((reach, angle_degrees)),
            _ => None,
        }) else {
            continue;
        };
        let mut queued_payloads = false;
        let fighter_filter = avian2d::prelude::SpatialQueryFilter::from_mask(FIGHTER_LAYER);
        let mut candidates: Vec<_> = spatial_query
            .shape_intersections(
                &Collider::circle(reach),
                attack.origin,
                0.0,
                &fighter_filter,
            )
            .into_iter()
            .filter_map(|entity| fighters.get(entity).ok())
            .collect();
        candidates.sort_by_key(|(_, _, _, network_id, _, _)| network_id.0);
        for (target, position, team, network_id, defeated, controlled) in candidates {
            if defeated.is_some()
                || controlled.is_some_and(|controlled| disconnected.contains(&controlled.owner))
                || !payload_target_visible(attack.source, *team, *network_id)
                || !delivery::sector_contains(
                    attack.origin,
                    attack.facing,
                    reach,
                    angle,
                    position.0,
                    tuning.radius,
                )
                || !area_line_of_sight_clear(attack.origin, position.0, &spatial_query)
            {
                continue;
            }
            let valid_bundles: Vec<_> = attack
                .recipe
                .payload_bundles
                .iter()
                .enumerate()
                .filter(|(_, bundle)| {
                    matches!(bundle.target, TargetSelection::Direct)
                        && payload_can_affect_target(bundle, attack.source, *team, *network_id)
                })
                .collect();
            if valid_bundles.is_empty() {
                continue;
            }
            deliveries.write(PendingDelivery {
                entity: None,
                source: attack.source,
                delivery_index: 0,
                tick: attack.tick,
                engagement_distance: attack.origin.distance(position.0),
                delivery_travel: 0.0,
                kind: PendingDeliveryKind::MeleeContact {
                    target: *network_id,
                    position: WorldPoint::from(position.0),
                },
            });
            for (bundle_index, bundle) in valid_bundles {
                pending.write(PendingPayload {
                    source: attack.source,
                    delivery_index: 0,
                    bundle_index: u8::try_from(bundle_index).unwrap_or(u8::MAX),
                    target,
                    target_network_id: *network_id,
                    position: position.0,
                    engagement_distance: attack.origin.distance(position.0),
                    delivery_travel: 0.0,
                    contact_fraction: 1.0,
                    bundle: bundle.clone(),
                });
                queued_payloads = true;
            }
        }
        if !queued_payloads {
            finish_attack_delivery(&mut trackers, attack.source.attack_id);
        }
    }
}

#[cfg(feature = "server")]
fn payload_target_visible(source: AttackSource, team: TeamId, network_id: NetworkEntityId) -> bool {
    network_id == source.owner_network_entity_id || teams_are_hostile(source.team_id, team)
}

#[cfg(feature = "server")]
fn pending_delivery_kind_order(kind: &PendingDeliveryKind) -> u8 {
    match kind {
        PendingDeliveryKind::StraightImpact { .. } => 0,
        PendingDeliveryKind::LobLanded { .. } => 1,
        PendingDeliveryKind::MeleeContact { .. } => 2,
    }
}

#[cfg(feature = "server")]
fn record_delivery_telemetry(
    telemetry: &mut WeaponTelemetry,
    delivery: &PendingDelivery,
    event_id: CombatEventId,
    target: Option<NetworkEntityId>,
    position: WorldPoint,
    outcome: WeaponTelemetryOutcome,
) {
    telemetry.record(WeaponTelemetryRecord {
        tick: delivery.tick,
        event_id,
        attack_id: delivery.source.attack_id,
        preset_id: delivery
            .source
            .source_preset_id
            .unwrap_or(WeaponPresetId(0)),
        recipe_fingerprint: delivery.source.recipe_fingerprint,
        delivery_index: Some(delivery.delivery_index),
        source: delivery.source.owner_network_entity_id,
        target,
        position,
        requested_value: 0,
        applied_value: 0,
        engagement_distance: delivery.engagement_distance,
        delivery_travel: delivery.delivery_travel,
        hostile_contact: target.is_some(),
        effect: None,
        resulting_health: None,
        resulting_effects: None,
        resulting_motion: None,
        outcome,
    });
}

#[cfg(feature = "server")]
fn abort_composed_event_batch(
    commands: &mut Commands,
    trackers: &mut ActiveAttackTrackers,
    deliveries: &[PendingDelivery],
    payloads: &[PendingPayload],
) {
    let mut affected_attacks = HashSet::new();
    for delivery in deliveries {
        affected_attacks.insert(delivery.source.attack_id);
        if let Some(entity) = delivery.entity {
            commands.entity(entity).despawn();
        }
    }
    for payload in payloads {
        affected_attacks.insert(payload.source.attack_id);
    }
    for attack_id in affected_attacks {
        trackers.active.remove(&attack_id);
    }
}

#[cfg(feature = "server")]
fn finish_attack_delivery(trackers: &mut ActiveAttackTrackers, attack_id: AttackId) {
    let Some(tracker) = trackers.active.get_mut(&attack_id) else {
        return;
    };
    tracker.resolved_deliveries = tracker
        .resolved_deliveries
        .saturating_add(1)
        .min(tracker.expected_deliveries);
    if tracker.resolved_deliveries >= tracker.expected_deliveries
        && let Some(tracker) = trackers.active.remove(&attack_id)
    {
        trackers.completed.push(CompletedAttack {
            source_preset_id: tracker.source.source_preset_id,
            recipe_fingerprint: tracker.source.recipe_fingerprint,
            had_hostile_contact: tracker.had_hostile_contact,
        });
    }
}

#[cfg(feature = "server")]
fn flush_completed_attack_telemetry(
    mut trackers: ResMut<ActiveAttackTrackers>,
    mut telemetry: ResMut<WeaponTelemetry>,
) {
    for completed in trackers.completed.drain(..) {
        telemetry.record_attack_completion(
            completed.source_preset_id.unwrap_or(WeaponPresetId(0)),
            completed.recipe_fingerprint,
            completed.had_hostile_contact,
        );
    }
}

#[cfg(feature = "server")]
fn effect_recipient_scale(
    effect: PayloadEffectDefinition,
    source: AttackSource,
    target_network_id: NetworkEntityId,
    target_team: TeamId,
) -> Option<f32> {
    let recipients = match effect {
        PayloadEffectDefinition::Damage { recipients, .. }
        | PayloadEffectDefinition::Knockback { recipients, .. }
        | PayloadEffectDefinition::Slow { recipients, .. } => recipients,
    };
    if target_network_id == source.owner_network_entity_id {
        match recipients {
            RecipientPolicy::HostilesAndOwner { owner_scale } => Some(owner_scale),
            RecipientPolicy::Hostiles => None,
        }
    } else if teams_are_hostile(source.team_id, target_team) {
        Some(1.0)
    } else {
        None
    }
}

#[cfg(feature = "server")]
fn resolve_composed_payloads(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    fighters: Res<FighterDefinitions>,
    mut ids: ResMut<NextCombatIds>,
    mut trackers: ResMut<ActiveAttackTrackers>,
    mut payloads: MessageReader<PendingPayload>,
    mut deliveries: MessageReader<PendingDelivery>,
    mut telemetry: ResMut<WeaponTelemetry>,
    mut legacy_telemetry: ResMut<CombatTelemetry>,
    mut outbox: ResMut<CombatOutbox>,
    mut target_queries: ParamSet<(
        Query<
            (
                &NetworkEntityId,
                &FighterDefinitionId,
                &TeamId,
                &mut CurrentHealth,
                Option<&mut ActiveEffects>,
                Option<&ExternalMotion>,
                Option<&Defeated>,
                Option<&lightyear::prelude::ControlledBy>,
            ),
            With<Fighter>,
        >,
        Query<
            (
                Entity,
                &NetworkEntityId,
                &TeamId,
                &CurrentHealth,
                Option<&Defeated>,
                Option<&lightyear::prelude::ControlledBy>,
            ),
            With<Fighter>,
        >,
    )>,
    owners: Query<(&NetworkEntityId, Option<&lightyear::prelude::ControlledBy>), With<Fighter>>,
    disconnected: Query<Entity, (With<LinkOf>, With<lightyear::prelude::Disconnected>)>,
) {
    let disconnected: HashSet<_> = disconnected.iter().collect();
    let connected_owners: HashSet<_> = owners
        .iter()
        .filter(|(_, controlled)| {
            controlled.is_none_or(|controlled| !disconnected.contains(&controlled.owner))
        })
        .map(|(network_id, _)| network_id.0)
        .collect();
    let mut records: Vec<_> = payloads.read().cloned().collect();
    records.sort_by(|left, right| {
        left.target_network_id
            .0
            .cmp(&right.target_network_id.0)
            .then_with(|| left.contact_fraction.total_cmp(&right.contact_fraction))
            .then_with(|| left.source.attack_id.0.cmp(&right.source.attack_id.0))
            .then_with(|| left.delivery_index.cmp(&right.delivery_index))
            .then_with(|| left.bundle_index.cmp(&right.bundle_index))
    });
    let mut delivery_records: Vec<_> = deliveries.read().cloned().collect();
    delivery_records.sort_by(|left, right| {
        left.source
            .attack_id
            .0
            .cmp(&right.source.attack_id.0)
            .then_with(|| left.delivery_index.cmp(&right.delivery_index))
            .then_with(|| left.tick.cmp(&right.tick))
            .then_with(|| {
                pending_delivery_kind_order(&left.kind)
                    .cmp(&pending_delivery_kind_order(&right.kind))
            })
    });

    // Dry-run the complete sorted batch against a snapshot before reserving any outcome IDs or
    // mutating health/effects. Event exhaustion must abort the whole batch, not leave an earlier
    // target partially committed while a later record fails to reserve its IDs.
    let mut planned_targets: HashMap<Entity, (NetworkEntityId, TeamId, u16, bool)> = target_queries
        .p1()
        .iter()
        .filter(|(_, _, _, _, _, controlled)| {
            controlled.is_none_or(|controlled| !disconnected.contains(&controlled.owner))
        })
        .map(|(entity, network_id, team, health, defeated, _)| {
            (entity, (*network_id, *team, health.0, defeated.is_some()))
        })
        .collect();
    let mut required_event_count = delivery_records
        .iter()
        .filter(|delivery| connected_owners.contains(&delivery.source.owner_network_entity_id.0))
        .map(|delivery| {
            1 + usize::from(
                delivery.source.legacy_compatibility
                    && matches!(delivery.kind, PendingDeliveryKind::StraightImpact { .. }),
            )
        })
        .sum::<usize>();
    for record in &records {
        if !connected_owners.contains(&record.source.owner_network_entity_id.0) {
            continue;
        }
        let Some((target_network_id, target_team, health, defeated)) =
            planned_targets.get_mut(&record.target)
        else {
            continue;
        };
        let legacy_compatibility = record.source.legacy_compatibility;
        let mut effects = record.bundle.effects.clone();
        effects.sort_by_key(|effect| {
            u8::from(!matches!(effect, PayloadEffectDefinition::Damage { .. }))
        });
        for effect in effects {
            let Some(scale) =
                effect_recipient_scale(effect, record.source, *target_network_id, *target_team)
            else {
                continue;
            };
            let event_count = match effect {
                PayloadEffectDefinition::Damage {
                    amount, falloff, ..
                } => {
                    let requested = (f32::from(amount)
                        * linear_falloff(falloff, record.delivery_travel)
                        * scale)
                        .round()
                        .max(1.0) as u16;
                    let applied = requested.min(*health);
                    if applied == 0 {
                        0
                    } else {
                        let defeats = !*defeated && *health == applied;
                        *health = (*health).saturating_sub(applied);
                        if defeats {
                            *defeated = true;
                            2 + usize::from(legacy_compatibility) * 2
                        } else {
                            1 + usize::from(legacy_compatibility)
                        }
                    }
                }
                PayloadEffectDefinition::Knockback { .. }
                | PayloadEffectDefinition::Slow { .. }
                    if !*defeated =>
                {
                    1
                }
                PayloadEffectDefinition::Knockback { .. }
                | PayloadEffectDefinition::Slow { .. } => 0,
            };
            required_event_count = required_event_count
                .checked_add(event_count)
                .ok_or(())
                .unwrap_or(usize::MAX);
            if required_event_count == usize::MAX {
                telemetry.event_reservation_drops =
                    telemetry.event_reservation_drops.saturating_add(1);
                abort_composed_event_batch(
                    &mut commands,
                    &mut trackers,
                    &delivery_records,
                    &records,
                );
                return;
            }
        }
    }
    let Some(reserved_events) = server::reserve_event_ids(&mut ids, required_event_count) else {
        telemetry.event_reservation_drops = telemetry.event_reservation_drops.saturating_add(1);
        abort_composed_event_batch(&mut commands, &mut trackers, &delivery_records, &records);
        return;
    };
    let mut reserved_events = reserved_events.into_iter();
    let mut targets = target_queries.p0();
    let mut contacted_deliveries = HashSet::new();
    let mut defeated_this_tick = HashSet::new();
    let mut accumulated_effects: HashMap<Entity, ActiveEffects> = HashMap::new();
    let mut accumulated_motion: HashMap<Entity, ExternalMotion> = HashMap::new();
    let mut deferred_effect_cues: Vec<(Entity, CombatCue)> = Vec::new();
    let mut resolved_delivery_keys = HashSet::new();
    for delivery in delivery_records {
        resolved_delivery_keys.insert((delivery.source.attack_id, delivery.delivery_index));
        if !connected_owners.contains(&delivery.source.owner_network_entity_id.0) {
            if let Some(entity) = delivery.entity {
                commands.entity(entity).despawn();
            }
            finish_attack_delivery(&mut trackers, delivery.source.attack_id);
            continue;
        }
        let event_id = reserved_events
            .next()
            .expect("delivery event reservation matches pending deliveries");
        let weapon_definition_id = WeaponDefinitionId(
            delivery
                .source
                .source_preset_id
                .map_or(0, |preset| preset.0),
        );
        match delivery.kind {
            PendingDeliveryKind::StraightImpact {
                target,
                position,
                normal,
                distance_band,
            } => {
                let cue = CombatCue::DeliveryImpact {
                    event_id,
                    tick: delivery.tick,
                    attack_id: delivery.source.attack_id,
                    delivery_index: delivery.delivery_index,
                    source: delivery.source.owner_network_entity_id,
                    weapon_definition_id,
                    presentation_profile_id: delivery.source.presentation_profile_id,
                    target,
                    position,
                    normal,
                    distance_band,
                };
                legacy_telemetry.record_cue(cue.clone());
                outbox.0.push(cue);
                record_delivery_telemetry(
                    &mut telemetry,
                    &delivery,
                    event_id,
                    target,
                    position,
                    WeaponTelemetryOutcome::DeliveryImpact,
                );
                if delivery.source.legacy_compatibility {
                    let legacy_event = reserved_events
                        .next()
                        .expect("legacy impact reservation matches delivery");
                    let legacy_source = ProjectileSource {
                        shot_id: ShotId(delivery.source.attack_id.0),
                        player_id: delivery.source.player_id,
                        owner_network_entity_id: delivery.source.owner_network_entity_id,
                        team_id: delivery.source.team_id,
                        weapon_definition_id: PULSE_SIDEARM_DEFINITION,
                    };
                    let legacy_cue = CombatCue::Impact {
                        event_id: legacy_event,
                        tick: delivery.tick,
                        source: delivery.source.owner_network_entity_id,
                        shot_id: legacy_source.shot_id,
                        weapon_definition_id: legacy_source.weapon_definition_id,
                        target,
                        position,
                        normal,
                        distance_band,
                    };
                    legacy_telemetry.record_cue(legacy_cue.clone());
                    legacy_telemetry.record(CombatLogRecord::Hit {
                        tick: delivery.tick,
                        event_id: legacy_event,
                        shot_id: legacy_source.shot_id,
                        source: delivery.source.owner_network_entity_id,
                        target,
                        weapon: legacy_source.weapon_definition_id,
                        position,
                        distance: delivery
                            .source
                            .origin
                            .as_vec2()
                            .distance(position.as_vec2()),
                        band: distance_band,
                    });
                    outbox.0.push(legacy_cue);
                }
            }
            PendingDeliveryKind::LobLanded { position } => {
                let cue = CombatCue::LobLanded {
                    event_id,
                    tick: delivery.tick,
                    attack_id: delivery.source.attack_id,
                    delivery_index: delivery.delivery_index,
                    source: delivery.source.owner_network_entity_id,
                    weapon_definition_id,
                    presentation_profile_id: delivery.source.presentation_profile_id,
                    position,
                };
                legacy_telemetry.record_cue(cue.clone());
                outbox.0.push(cue);
                record_delivery_telemetry(
                    &mut telemetry,
                    &delivery,
                    event_id,
                    None,
                    position,
                    WeaponTelemetryOutcome::DeliveryLanding,
                );
            }
            PendingDeliveryKind::MeleeContact { target, position } => {
                let cue = CombatCue::MeleeContact {
                    event_id,
                    tick: delivery.tick,
                    attack_id: delivery.source.attack_id,
                    delivery_index: delivery.delivery_index,
                    source: delivery.source.owner_network_entity_id,
                    weapon_definition_id,
                    presentation_profile_id: delivery.source.presentation_profile_id,
                    target,
                    position,
                };
                legacy_telemetry.record_cue(cue.clone());
                outbox.0.push(cue);
                record_delivery_telemetry(
                    &mut telemetry,
                    &delivery,
                    event_id,
                    Some(target),
                    position,
                    WeaponTelemetryOutcome::MeleeContact,
                );
            }
        }
        if let Some(entity) = delivery.entity {
            commands.entity(entity).despawn();
        }
    }
    for record in records {
        resolved_delivery_keys.insert((record.source.attack_id, record.delivery_index));
        if !connected_owners.contains(&record.source.owner_network_entity_id.0) {
            trackers.active.remove(&record.source.attack_id);
            continue;
        }
        let Ok((
            target_network_id,
            fighter_id,
            target_team,
            mut health,
            active_effects,
            external_motion,
            defeated,
            controlled_by,
        )) = targets.get_mut(record.target)
        else {
            continue;
        };
        if controlled_by.is_some_and(|controlled| disconnected.contains(&controlled.owner)) {
            continue;
        }
        let mut effects_state = accumulated_effects
            .get(&record.target)
            .copied()
            .unwrap_or_else(|| {
                active_effects.map_or_else(ActiveEffects::default, |effects| *effects)
            });
        let mut motion_state = accumulated_motion
            .get(&record.target)
            .copied()
            .or(external_motion.copied());
        let preset_id = record.source.source_preset_id.unwrap_or(WeaponPresetId(0));
        let legacy_compatibility = record.source.legacy_compatibility;
        let source = DamageSource::PlayerWeapon {
            player_id: record.source.player_id,
            fighter_id: record.source.owner_network_entity_id,
            weapon_definition_id: WeaponDefinitionId(preset_id.0),
            shot_id: ShotId(record.source.attack_id.0),
        };
        let mut target_defeated = defeated.is_some() || defeated_this_tick.contains(&record.target);
        let owner_contact = *target_network_id == record.source.owner_network_entity_id;
        if !owner_contact
            && !target_defeated
            && teams_are_hostile(record.source.team_id, *target_team)
            && contacted_deliveries.insert((
                record.source.attack_id,
                record.delivery_index,
                target_network_id.0,
            ))
        {
            telemetry.record_hostile_delivery_contact(preset_id, record.source.recipe_fingerprint);
            if let Some(tracker) = trackers.active.get_mut(&record.source.attack_id) {
                tracker.had_hostile_contact = true;
            }
        }
        let mut effects = record.bundle.effects.clone();
        effects.sort_by_key(|effect| {
            u8::from(!matches!(effect, PayloadEffectDefinition::Damage { .. }))
        });
        let mut projected_health = health.0;
        let mut projected_defeated = target_defeated;
        for effect in effects.iter().copied() {
            let Some(scale) =
                effect_recipient_scale(effect, record.source, *target_network_id, *target_team)
            else {
                continue;
            };
            match effect {
                PayloadEffectDefinition::Damage {
                    amount, falloff, ..
                } => {
                    let requested = (f32::from(amount)
                        * linear_falloff(falloff, record.delivery_travel)
                        * scale)
                        .round()
                        .max(1.0) as u16;
                    let applied = requested.min(projected_health);
                    if applied > 0 {
                        // IDs for the complete batch were reserved by the dry-run above.
                        if !projected_defeated && projected_health == applied {
                            projected_defeated = true;
                        }
                        projected_health = projected_health.saturating_sub(applied);
                    }
                }
                PayloadEffectDefinition::Knockback { .. }
                | PayloadEffectDefinition::Slow { .. }
                    if !projected_defeated => {}
                PayloadEffectDefinition::Knockback { .. }
                | PayloadEffectDefinition::Slow { .. } => {}
            }
        }
        for effect in effects.iter().copied() {
            let Some(scale) =
                effect_recipient_scale(effect, record.source, *target_network_id, *target_team)
            else {
                continue;
            };
            if let PayloadEffectDefinition::Damage {
                amount,
                falloff,
                recipients,
            } = effect
            {
                let requested =
                    (f32::from(amount) * linear_falloff(falloff, record.delivery_travel) * scale)
                        .round()
                        .max(1.0) as u16;
                let applied = requested.min(health.0);
                if applied == 0 {
                    continue;
                }
                let defeats = applied > 0 && !target_defeated && health.0 == applied;
                let damage_event = reserved_events
                    .next()
                    .expect("complete payload event reservation matches damage");
                let legacy_damage_event = legacy_compatibility.then(|| {
                    reserved_events
                        .next()
                        .expect("payload event reservation matches legacy damage")
                });
                let defeat_event = if defeats {
                    Some(
                        reserved_events
                            .next()
                            .expect("payload event reservation matches defeat"),
                    )
                } else {
                    None
                };
                let legacy_defeat_event = if defeats && legacy_compatibility {
                    Some(
                        reserved_events
                            .next()
                            .expect("payload event reservation matches legacy defeat"),
                    )
                } else {
                    None
                };
                health.0 = health.0.saturating_sub(applied);
                if applied > 0 {
                    let owner_damage = *target_network_id == record.source.owner_network_entity_id;
                    let band = distance_band(record.engagement_distance);
                    telemetry.record_damage(
                        preset_id,
                        record.source.recipe_fingerprint,
                        owner_damage,
                        band,
                        applied,
                    );
                    legacy_telemetry.applied_damage = legacy_telemetry
                        .applied_damage
                        .saturating_add(u64::from(applied));
                    if owner_damage {
                        legacy_telemetry.close_hits = legacy_telemetry.close_hits.saturating_add(1);
                    } else {
                        legacy_telemetry.hostile_fighter_hits =
                            legacy_telemetry.hostile_fighter_hits.saturating_add(1);
                    }
                    let damage_cue = CombatCue::DamageApplied {
                        event_id: damage_event,
                        tick: tick.0,
                        attack_id: record.source.attack_id,
                        source,
                        target: *target_network_id,
                        amount: applied,
                        health_after: health.0,
                        distance_band: distance_band(record.engagement_distance),
                        presentation_profile_id: record.source.presentation_profile_id,
                    };
                    legacy_telemetry.record_cue(damage_cue.clone());
                    outbox.0.push(damage_cue);
                    if let Some(legacy_damage_event) = legacy_damage_event {
                        let legacy_cue = CombatCue::Damage {
                            event_id: legacy_damage_event,
                            tick: tick.0,
                            source,
                            target: *target_network_id,
                            amount: applied,
                            health_after: health.0,
                            distance_band: distance_band(record.engagement_distance),
                        };
                        legacy_telemetry.record_cue(legacy_cue.clone());
                        legacy_telemetry.record(CombatLogRecord::Damage {
                            tick: tick.0,
                            event_id: legacy_damage_event,
                            source,
                            target: *target_network_id,
                            requested,
                            applied,
                            health_after: health.0,
                        });
                        outbox.0.push(legacy_cue);
                    }
                    telemetry.record(WeaponTelemetryRecord {
                        tick: tick.0,
                        event_id: damage_event,
                        attack_id: record.source.attack_id,
                        preset_id,
                        recipe_fingerprint: record.source.recipe_fingerprint,
                        delivery_index: Some(record.delivery_index),
                        source: record.source.owner_network_entity_id,
                        target: Some(*target_network_id),
                        position: WorldPoint::from(record.position),
                        requested_value: requested,
                        applied_value: applied,
                        engagement_distance: record.engagement_distance,
                        delivery_travel: record.delivery_travel,
                        hostile_contact: !owner_damage,
                        effect: Some(PayloadEffectDefinition::Damage {
                            amount,
                            falloff,
                            recipients,
                        }),
                        resulting_health: Some(health.0),
                        resulting_effects: Some(effects_state),
                        resulting_motion: motion_state,
                        outcome: WeaponTelemetryOutcome::DamageApplied,
                    });
                }
                if let Some(defeat_event) = defeat_event {
                    defeated_this_tick.insert(record.target);
                    target_defeated = true;
                    let reset_at_tick = tick.0.saturating_add(
                        fighters
                            .get(*fighter_id)
                            .map_or(90, |definition| definition.defeat_reset_delay_ticks),
                    );
                    commands
                        .entity(record.target)
                        .insert((
                            Defeated {
                                event_id: defeat_event,
                                reset_at_tick,
                            },
                            CollisionLayers::new(FIGHTER_LAYER, avian2d::prelude::LayerMask::NONE),
                            ActiveEffects::default(),
                        ))
                        .remove::<ExternalMotion>()
                        .remove::<KnockbackFeedback>();
                    accumulated_effects.remove(&record.target);
                    accumulated_motion.remove(&record.target);
                    telemetry.record_defeat(preset_id, record.source.recipe_fingerprint);
                    telemetry.record(WeaponTelemetryRecord {
                        tick: tick.0,
                        event_id: defeat_event,
                        attack_id: record.source.attack_id,
                        preset_id,
                        recipe_fingerprint: record.source.recipe_fingerprint,
                        delivery_index: Some(record.delivery_index),
                        source: record.source.owner_network_entity_id,
                        target: Some(*target_network_id),
                        position: WorldPoint::from(record.position),
                        requested_value: 0,
                        applied_value: 0,
                        engagement_distance: record.engagement_distance,
                        delivery_travel: record.delivery_travel,
                        hostile_contact: !owner_contact,
                        effect: None,
                        resulting_health: Some(0),
                        resulting_effects: Some(ActiveEffects::default()),
                        resulting_motion: None,
                        outcome: WeaponTelemetryOutcome::Defeat,
                    });
                    legacy_telemetry.defeats = legacy_telemetry.defeats.saturating_add(1);
                    let defeated_cue = CombatCue::FighterDefeated {
                        event_id: defeat_event,
                        tick: tick.0,
                        attack_id: record.source.attack_id,
                        source: Some(source),
                        target: *target_network_id,
                        presentation_profile_id: Some(record.source.presentation_profile_id),
                    };
                    legacy_telemetry.record_cue(defeated_cue.clone());
                    outbox.0.push(defeated_cue);
                    if let Some(legacy_defeat_event) = legacy_defeat_event {
                        let legacy_cue = CombatCue::Defeat {
                            event_id: legacy_defeat_event,
                            tick: tick.0,
                            source: Some(source),
                            target: *target_network_id,
                        };
                        legacy_telemetry.record_cue(legacy_cue.clone());
                        legacy_telemetry.record(CombatLogRecord::Defeat {
                            tick: tick.0,
                            event_id: legacy_defeat_event,
                            source: Some(source),
                            target: *target_network_id,
                        });
                        outbox.0.push(legacy_cue);
                    }
                }
            }
        }
        if target_defeated {
            accumulated_effects.remove(&record.target);
            accumulated_motion.remove(&record.target);
            continue;
        }
        for effect in record.bundle.effects.iter().copied() {
            let Some(scale) =
                effect_recipient_scale(effect, record.source, *target_network_id, *target_team)
            else {
                continue;
            };
            match effect {
                PayloadEffectDefinition::Knockback {
                    speed,
                    duration_ticks,
                    recipients,
                } => {
                    let direction =
                        (record.position - record.source.origin.as_vec2()).normalize_or_zero();
                    let motion = effects::combine_knockback(
                        motion_state,
                        direction * speed * scale,
                        tick.0.saturating_add(duration_ticks),
                    );
                    motion_state = Some(motion);
                    let event_id = reserved_events
                        .next()
                        .expect("payload event reservation matches knockback");
                    let effect_cue = CombatCue::EffectApplied {
                        event_id,
                        tick: tick.0,
                        attack_id: record.source.attack_id,
                        source,
                        target: *target_network_id,
                        effect: CombatEffectCue::Knockback {
                            velocity: WorldPoint::from(motion.velocity),
                            expires_at_tick: motion.expires_at_tick,
                        },
                        presentation_profile_id: record.source.presentation_profile_id,
                    };
                    telemetry.record(WeaponTelemetryRecord {
                        tick: tick.0,
                        event_id,
                        attack_id: record.source.attack_id,
                        preset_id,
                        recipe_fingerprint: record.source.recipe_fingerprint,
                        delivery_index: Some(record.delivery_index),
                        source: record.source.owner_network_entity_id,
                        target: Some(*target_network_id),
                        position: WorldPoint::from(record.position),
                        requested_value: 0,
                        applied_value: 0,
                        engagement_distance: record.engagement_distance,
                        delivery_travel: record.delivery_travel,
                        hostile_contact: !owner_contact,
                        effect: Some(PayloadEffectDefinition::Knockback {
                            speed,
                            duration_ticks,
                            recipients,
                        }),
                        resulting_health: Some(health.0),
                        resulting_effects: Some(effects_state),
                        resulting_motion: Some(motion),
                        outcome: WeaponTelemetryOutcome::KnockbackApplied,
                    });
                    deferred_effect_cues.push((record.target, effect_cue));
                }
                PayloadEffectDefinition::Slow {
                    movement_multiplier,
                    duration_ticks,
                    stacking,
                    recipients,
                } => {
                    effects::refresh_strongest_slow(
                        &mut effects_state,
                        record.source.attack_id,
                        record.source.owner_network_entity_id,
                        (movement_multiplier * scale * 1000.0)
                            .round()
                            .clamp(1.0, 1000.0) as u16,
                        tick.0.saturating_add(duration_ticks),
                    );
                    if let Some(slow) = effects_state.slow {
                        let event_id = reserved_events
                            .next()
                            .expect("payload event reservation matches slow");
                        let effect_cue = CombatCue::EffectApplied {
                            event_id,
                            tick: tick.0,
                            attack_id: record.source.attack_id,
                            source,
                            target: *target_network_id,
                            effect: CombatEffectCue::Slow {
                                movement_multiplier_milli: slow.movement_multiplier_milli,
                                expires_at_tick: slow.expires_at_tick,
                            },
                            presentation_profile_id: record.source.presentation_profile_id,
                        };
                        telemetry.record(WeaponTelemetryRecord {
                            tick: tick.0,
                            event_id,
                            attack_id: record.source.attack_id,
                            preset_id,
                            recipe_fingerprint: record.source.recipe_fingerprint,
                            delivery_index: Some(record.delivery_index),
                            source: record.source.owner_network_entity_id,
                            target: Some(*target_network_id),
                            position: WorldPoint::from(record.position),
                            requested_value: 0,
                            applied_value: 0,
                            engagement_distance: record.engagement_distance,
                            delivery_travel: record.delivery_travel,
                            hostile_contact: !owner_contact,
                            effect: Some(PayloadEffectDefinition::Slow {
                                movement_multiplier,
                                duration_ticks,
                                stacking,
                                recipients,
                            }),
                            resulting_health: Some(health.0),
                            resulting_effects: Some(effects_state),
                            resulting_motion: motion_state,
                            outcome: WeaponTelemetryOutcome::SlowApplied,
                        });
                        deferred_effect_cues.push((record.target, effect_cue));
                    }
                }
                PayloadEffectDefinition::Damage { .. } => {}
            }
        }
        accumulated_effects.insert(record.target, effects_state);
        if let Some(motion) = motion_state {
            accumulated_motion.insert(record.target, motion);
        }
    }
    for (entity, effects) in accumulated_effects {
        if !defeated_this_tick.contains(&entity) {
            commands.entity(entity).insert(effects);
        }
    }
    for (entity, motion) in accumulated_motion {
        if !defeated_this_tick.contains(&entity) {
            commands.entity(entity).insert((
                motion,
                KnockbackFeedback {
                    velocity: WorldPoint::from(motion.velocity),
                    expires_at_tick: motion.expires_at_tick,
                },
            ));
        }
    }
    for (entity, cue) in deferred_effect_cues {
        if !defeated_this_tick.contains(&entity) {
            legacy_telemetry.record_cue(cue.clone());
            outbox.0.push(cue);
        }
    }
    for (attack_id, _) in resolved_delivery_keys {
        finish_attack_delivery(&mut trackers, attack_id);
    }
}

#[cfg(feature = "server")]
#[derive(Resource, Default, Debug)]
pub struct CombatOutbox(pub Vec<CombatCue>);

#[cfg(feature = "server")]
#[derive(Resource, Default, Debug)]
pub struct CombatEvidenceSnapshots {
    pub checkpoints: BTreeMap<String, CombatStateSnapshot>,
    pub checkpoint_candidates: BTreeMap<String, Vec<(CombatStateSnapshot, u128)>>,
    checkpoint_latched_snapshots: BTreeMap<String, CombatStateSnapshot>,
    checkpoint_latch_ticks: BTreeMap<String, u16>,
    pub checkpoint_timestamps: BTreeMap<String, u128>,
    pub state_mutation_timestamps: Vec<(u64, u128)>,
    pub last_encoded_snapshot: Option<String>,
    pub saw_defeat: bool,
    pub pending_checkpoints: Vec<CombatEvidenceCheckpoint>,
}

#[cfg(any(feature = "server", feature = "client"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatCheckpoint {
    ActiveScatterFlight,
    ActiveLobFlight,
    ActiveSlow,
    ActiveKnockback,
    Defeat,
    Reset,
}

#[cfg(any(feature = "server", feature = "client"))]
impl CombatCheckpoint {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ActiveScatterFlight => "active_scatter_flight",
            Self::ActiveLobFlight => "active_lob_flight",
            Self::ActiveSlow => "active_slow",
            Self::ActiveKnockback => "active_knockback",
            Self::Defeat => "defeat",
            Self::Reset => "reset",
        }
    }
}

#[cfg(any(feature = "server", feature = "client"))]
#[derive(Message, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CombatEvidenceCheckpoint {
    pub checkpoint: CombatCheckpoint,
    pub snapshot: CombatStateSnapshot,
}

#[cfg(feature = "server")]
fn capture_server_combat_checkpoints(
    tick: Res<SimulationTick>,
    mut evidence: ResMut<CombatEvidenceSnapshots>,
    fighters: Query<
        (
            &NetworkEntityId,
            Option<&SelectedBuild>,
            Option<&ResolvedWeapon>,
            Option<&WeaponState>,
            Option<&ActiveEffects>,
            Option<&Defeated>,
            Option<&ExternalMotion>,
            Option<&KnockbackFeedback>,
            Option<&CurrentHealth>,
            Option<&AuthoritativePose>,
            &Position,
            &Rotation,
            Option<&AuthoritativeTick>,
        ),
        With<Fighter>,
    >,
    projectiles: Query<
        (
            &Position,
            Option<&AttackDelivery>,
            Option<&ReplicatedAttackSource>,
            Option<&LobbedFlight>,
            Option<&ProjectileDeadline>,
            Option<&StraightFlight>,
        ),
        With<Projectile>,
    >,
) {
    if !env::var("BRAWLER_NETWORK_ASSERT_COMBAT").is_ok_and(|value| value == "1") {
        return;
    }
    let expired_latches = evidence
        .checkpoint_latch_ticks
        .iter_mut()
        .filter_map(|(checkpoint, ticks)| {
            if *ticks == 0 {
                Some(checkpoint.clone())
            } else {
                *ticks = ticks.saturating_sub(1);
                None
            }
        })
        .collect::<Vec<_>>();
    for checkpoint in expired_latches {
        evidence.checkpoint_latch_ticks.remove(&checkpoint);
        evidence.checkpoint_latched_snapshots.remove(&checkpoint);
    }
    let mut fighter_snapshots = fighters
        .iter()
        .map(
            |(
                network_id,
                selected_build,
                resolved_weapon,
                weapon_state,
                active_effects,
                defeated,
                _external_motion,
                knockback_feedback,
                health,
                authoritative_pose,
                position,
                rotation,
                authoritative_tick,
            )| {
                CombatFighterSnapshot {
                    network_entity_id: *network_id,
                    selected_build: selected_build.copied(),
                    resolved_weapon: resolved_weapon.cloned(),
                    weapon_state: weapon_state.copied(),
                    active_effects: active_effects.copied(),
                    knockback_feedback: knockback_feedback.copied(),
                    defeated: defeated.copied(),
                    health: health.map(|health| health.0),
                    position: authoritative_pose
                        .map_or_else(|| WorldPoint::from(position.0), |pose| pose.position),
                    facing: authoritative_pose
                        .map_or_else(|| rotation.as_radians(), |pose| pose.facing),
                    authoritative_tick: authoritative_tick.copied().unwrap_or_default(),
                }
            },
        )
        .collect::<Vec<_>>();
    fighter_snapshots.sort_by_key(|fighter| fighter.network_entity_id.0);
    let mut projectile_snapshots = projectiles
        .iter()
        .filter_map(
            |(position, delivery, source, lobbed_flight, deadline, straight_flight)| {
                CombatProjectileSnapshot::from_components(
                    WorldPoint::from(position.0),
                    delivery,
                    source,
                    lobbed_flight,
                    deadline,
                    straight_flight,
                    tick.0,
                )
            },
        )
        .collect::<Vec<_>>();
    projectile_snapshots
        .sort_by_key(|projectile| (projectile.attack_id.0, projectile.delivery_index));
    let snapshot = CombatStateSnapshot {
        authoritative_tick: tick.0,
        fighters: fighter_snapshots,
        projectiles: projectile_snapshots,
    };
    let Some(encoded) = encode_state_snapshot(&snapshot) else {
        return;
    };
    if evidence.last_encoded_snapshot.as_deref() != Some(encoded.as_str()) {
        if evidence.state_mutation_timestamps.len() < MAX_COMBAT_EVIDENCE_EVENTS {
            evidence
                .state_mutation_timestamps
                .push((tick.0, unix_epoch_micros()));
        }
        evidence.last_encoded_snapshot = Some(encoded);
    }
    let has_scatter_flight = snapshot.projectiles.iter().any(|projectile| {
        projectile.presentation_profile_id == Some(WeaponPresentationProfileId(2))
            && projectile.lobbed_flight.is_none()
    });
    let has_lob_flight = snapshot
        .projectiles
        .iter()
        .any(|projectile| projectile.lobbed_flight.is_some());
    let has_slow = snapshot.fighters.iter().any(|fighter| {
        fighter
            .active_effects
            .is_some_and(|effects| effects.slow.is_some())
    });
    let has_defeat = snapshot
        .fighters
        .iter()
        .any(|fighter| fighter.defeated.is_some());
    evidence.saw_defeat |= has_defeat;
    let has_knockback = fighters
        .iter()
        .any(|(_, _, _, _, _, _, external_motion, _, _, _, _, _, _)| external_motion.is_some());
    let has_reset = evidence.saw_defeat
        && snapshot.fighters.iter().any(|fighter| {
            fighter.network_entity_id == DUMMY_NETWORK_ENTITY && fighter.defeated.is_none()
        });
    for (checkpoint, active) in [
        ("active_scatter_flight", has_scatter_flight),
        ("active_lob_flight", has_lob_flight),
        ("active_slow", has_slow),
        ("active_knockback", has_knockback),
        ("defeat", has_defeat),
        ("reset", has_reset),
    ] {
        let repeat_checkpoint =
            checkpoint.starts_with("active_") || checkpoint == "defeat" || checkpoint == "reset";
        let latched = evidence
            .checkpoint_latched_snapshots
            .contains_key(checkpoint);
        if (active || latched)
            && (!evidence.checkpoints.contains_key(checkpoint) || repeat_checkpoint)
        {
            let current_snapshot = if matches!(checkpoint, "defeat" | "reset") {
                CombatStateSnapshot {
                    projectiles: Vec::new(),
                    ..snapshot.clone()
                }
            } else {
                snapshot.clone()
            };
            if active && !latched {
                evidence
                    .checkpoint_latched_snapshots
                    .insert(checkpoint.to_string(), current_snapshot.clone());
                evidence
                    .checkpoint_latch_ticks
                    .insert(checkpoint.to_string(), COMBAT_CHECKPOINT_LATCH_TICKS);
            }
            let checkpoint_snapshot = evidence
                .checkpoint_latched_snapshots
                .get(checkpoint)
                .cloned()
                .unwrap_or(current_snapshot);
            let timestamp = unix_epoch_micros();
            let candidates = evidence
                .checkpoint_candidates
                .entry(checkpoint.to_string())
                .or_default();
            if candidates.len() < MAX_COMBAT_EVIDENCE_EVENTS {
                candidates.push((checkpoint_snapshot.clone(), timestamp));
            }
            if !evidence.checkpoints.contains_key(checkpoint) {
                evidence
                    .checkpoint_timestamps
                    .insert(checkpoint.to_string(), timestamp);
                evidence
                    .checkpoints
                    .insert(checkpoint.to_string(), checkpoint_snapshot.clone());
            }
            if evidence.pending_checkpoints.len() >= MAX_COMBAT_EVIDENCE_EVENTS {
                continue;
            }
            let checkpoint = match checkpoint {
                "active_scatter_flight" => CombatCheckpoint::ActiveScatterFlight,
                "active_lob_flight" => CombatCheckpoint::ActiveLobFlight,
                "active_slow" => CombatCheckpoint::ActiveSlow,
                "active_knockback" => CombatCheckpoint::ActiveKnockback,
                "defeat" => CombatCheckpoint::Defeat,
                "reset" => CombatCheckpoint::Reset,
                _ => unreachable!("combat evidence checkpoint is a known name"),
            };
            evidence.pending_checkpoints.push(CombatEvidenceCheckpoint {
                checkpoint,
                snapshot: checkpoint_snapshot,
            });
        }
    }
}

#[cfg(feature = "server")]
fn send_combat_evidence_checkpoints(
    mut evidence: ResMut<CombatEvidenceSnapshots>,
    mut senders: Query<
        &mut lightyear::prelude::MessageSender<CombatEvidenceCheckpoint>,
        With<LinkOf>,
    >,
) {
    let checkpoints = std::mem::take(&mut evidence.pending_checkpoints);
    for mut sender in &mut senders {
        for checkpoint in &checkpoints {
            sender.send::<crate::protocol::CombatChannel>(checkpoint.clone());
        }
    }
}

#[cfg(feature = "server")]
fn publish_authoritative_tick(
    tick: Res<SimulationTick>,
    mut fighters: Query<&mut AuthoritativeTick, With<Fighter>>,
) {
    for mut authoritative_tick in &mut fighters {
        authoritative_tick.0 = tick.0;
    }
}

#[cfg(feature = "server")]
fn cleanup_disconnected_projectiles(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    mut ids: ResMut<NextCombatIds>,
    mut trackers: ResMut<ActiveAttackTrackers>,
    mut telemetry: ResMut<WeaponTelemetry>,
    disconnected: Query<Entity, (With<LinkOf>, With<lightyear::prelude::Disconnected>)>,
    fighters: Query<(Entity, Option<&lightyear::prelude::ControlledBy>), With<Fighter>>,
    projectiles: Query<(Entity, &Position, &ComposedProjectileRuntime)>,
) {
    let disconnected: HashSet<_> = disconnected.iter().collect();
    let mut fighter_entities = HashSet::new();
    let mut disconnected_fighters = HashSet::new();
    for (fighter, controlled) in &fighters {
        fighter_entities.insert(fighter);
        if controlled.is_some_and(|controlled| disconnected.contains(&controlled.owner)) {
            disconnected_fighters.insert(fighter);
        }
    }
    for (entity, position, composed) in &projectiles {
        let owner_entity = composed.owner_entity;
        if disconnected_fighters.contains(&owner_entity)
            || !fighter_entities.contains(&owner_entity)
        {
            record_delivery_termination(
                &mut ids,
                &mut telemetry,
                tick.0,
                composed,
                position.0,
                WeaponTelemetryOutcome::DeliveryCancelled,
            );
            finish_attack_delivery(&mut trackers, composed.source.attack_id);
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(feature = "server")]
fn send_combat_cues(
    mut outbox: ResMut<CombatOutbox>,
    mut telemetry: ResMut<CombatTelemetry>,
    mut senders: Query<&mut lightyear::prelude::MessageSender<CombatCue>, With<LinkOf>>,
) {
    // Deferred effect cues can be created after a later target's damage cue. Keep the retained
    // process evidence in the same event order as the wire batch sent to clients.
    telemetry
        .cues
        .sort_by_key(|cue| combat_cue_key(cue).event_id.0);
    let mut cues = std::mem::take(&mut outbox.0);
    cues.sort_by_key(|cue| combat_cue_key(cue).event_id.0);
    for mut sender in &mut senders {
        for cue in &cues {
            sender.send::<crate::protocol::CombatChannel>(cue.clone());
        }
    }
}

#[cfg(feature = "server")]
fn emit_combat_summary(
    mut summary_logged: ResMut<CombatSummaryLogged>,
    telemetry: Res<CombatTelemetry>,
    weapon_telemetry: Res<WeaponTelemetry>,
    stopped: Query<(), (With<NetcodeServer>, With<Stopped>)>,
) {
    if summary_logged.0 || stopped.iter().next().is_none() {
        return;
    }
    summary_logged.0 = true;
    let hit_rate_basis_points = telemetry
        .hostile_fighter_hits
        .saturating_mul(10_000)
        .checked_div(telemetry.accepted_shots)
        .unwrap_or(0);
    info!(
        shots = telemetry.accepted_shots,
        hostile_hits = telemetry.hostile_fighter_hits,
        hit_rate_basis_points,
        applied_damage = telemetry.applied_damage,
        defeats = telemetry.defeats,
        close_hits = telemetry.close_hits,
        mid_hits = telemetry.mid_hits,
        long_hits = telemetry.long_hits,
        dropped_cues = telemetry.dropped_cues,
        dropped_records = telemetry.dropped_records,
        dropped_accepted_shot_timestamps = telemetry.dropped_accepted_shot_timestamps,
        weapon_dropped_records = weapon_telemetry.dropped_records,
        weapon_dropped_aggregate_entries = weapon_telemetry.dropped_aggregate_entries,
        "combat telemetry summary"
    );
}

#[cfg(feature = "client")]
#[derive(Resource, Default, Debug)]
struct RecentCombatEvents {
    ids: VecDeque<CombatEventId>,
}

/// Lets deterministic network tests consume the wire cue stream themselves instead of having
/// the presentation system drain it first.
#[cfg(feature = "client")]
#[derive(Resource, Debug, Default)]
pub struct CaptureCombatCues {
    pub cues: Vec<CombatCue>,
    pub dropped_cues: u64,
}

#[cfg(feature = "client")]
fn remember_combat_event(recent: &mut RecentCombatEvents, event_id: CombatEventId) -> bool {
    if recent.ids.contains(&event_id) {
        return false;
    }
    recent.ids.push_back(event_id);
    if recent.ids.len() > 256 {
        recent.ids.pop_front();
    }
    true
}

#[cfg(feature = "client")]
#[derive(Resource, Debug)]
struct ClientCombatObservation {
    saw_defeat: bool,
    saw_reset: bool,
    cue_timestamps: Vec<(ShotId, u128)>,
    cue_stream: Vec<CombatCue>,
    dropped_cue_timestamps: u64,
    dropped_cue_stream: u64,
    checkpoints: BTreeMap<String, CombatStateSnapshot>,
    checkpoint_matches: BTreeMap<String, Vec<CombatStateSnapshot>>,
    expected_checkpoints: Vec<CombatEvidenceCheckpoint>,
    snapshot_history: BTreeMap<u64, CombatStateSnapshot>,
    checkpoint_timestamps: BTreeMap<String, u128>,
    state_mutation_timestamps: Vec<(u64, u128)>,
    last_encoded_snapshot: Option<String>,
    ready_file: Option<PathBuf>,
    started_at: Instant,
    wrote_ready: bool,
}

#[cfg(feature = "client")]
impl FromWorld for ClientCombatObservation {
    fn from_world(_: &mut World) -> Self {
        let ready_file = env::var_os("BRAWLER_NETWORK_COMBAT_CLIENT_READY_FILE").map(PathBuf::from);
        Self {
            saw_defeat: false,
            saw_reset: false,
            cue_timestamps: Vec::new(),
            cue_stream: Vec::new(),
            dropped_cue_timestamps: 0,
            dropped_cue_stream: 0,
            checkpoints: BTreeMap::new(),
            checkpoint_matches: BTreeMap::new(),
            expected_checkpoints: Vec::new(),
            snapshot_history: BTreeMap::new(),
            checkpoint_timestamps: BTreeMap::new(),
            state_mutation_timestamps: Vec::new(),
            last_encoded_snapshot: None,
            ready_file,
            started_at: Instant::now(),
            wrote_ready: false,
        }
    }
}

#[cfg(feature = "client")]
#[derive(Component)]
struct CombatEffect {
    timer: Timer,
}

#[cfg(feature = "client")]
#[derive(Component)]
struct CombatHealthBar {
    target: Entity,
    fill: bool,
}

#[cfg(feature = "client")]
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct CombatStatusMarker {
    target: Entity,
    kind: CombatStatusKind,
}

#[cfg(feature = "client")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum CombatStatusKind {
    Slow,
    Knockback,
}

#[cfg(feature = "client")]
#[derive(Component)]
pub struct CombatHudText;

#[cfg(feature = "client")]
#[derive(Component)]
pub struct WeaponSelectionText;

#[cfg(feature = "client")]
pub struct ClientCombatPlugin;

#[cfg(feature = "client")]
impl Plugin for ClientCombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FighterDefinitions>()
            .init_resource::<WeaponDefinitions>()
            .init_resource::<RecentCombatEvents>()
            .init_resource::<ClientCombatObservation>()
            .add_systems(Startup, validate_definitions)
            .add_systems(
                Update,
                (
                    receive_combat_cues,
                    receive_combat_evidence_checkpoints,
                    ensure_projectile_visuals,
                    sync_projectile_visuals,
                    update_weapon_preview,
                    update_health_bars,
                    update_durable_effect_markers,
                    update_combat_hud,
                    update_combat_effects,
                    capture_client_combat_checkpoints,
                    record_headless_combat_observation,
                )
                    .chain(),
            );
    }
}

#[cfg(feature = "client")]
fn combat_cue_profile_id(cue: &CombatCue) -> u16 {
    match cue {
        CombatCue::AttackAccepted {
            presentation_profile_id,
            ..
        }
        | CombatCue::DeliveryImpact {
            presentation_profile_id,
            ..
        }
        | CombatCue::LobLanded {
            presentation_profile_id,
            ..
        }
        | CombatCue::MeleeContact {
            presentation_profile_id,
            ..
        }
        | CombatCue::DamageApplied {
            presentation_profile_id,
            ..
        }
        | CombatCue::EffectApplied {
            presentation_profile_id,
            ..
        } => presentation_profile_id.0,
        CombatCue::FighterDefeated {
            presentation_profile_id,
            ..
        } => presentation_profile_id.map_or(1, |profile| profile.0),
        _ => 1,
    }
}

#[cfg(feature = "client")]
fn combat_profile_color(profile_id: u16, fallback: Color) -> Color {
    match profile_id {
        2 => Color::srgb(1.0, 0.45, 0.12),
        3 => Color::srgb(0.25, 0.7, 1.0),
        4 => Color::srgb(0.85, 0.25, 1.0),
        _ => fallback,
    }
}

#[cfg(feature = "client")]
fn combat_profile_size(profile_id: u16, fallback: Vec2) -> Vec2 {
    match profile_id {
        2 => fallback * 0.8,
        3 => fallback * 1.25,
        4 => fallback * 1.1,
        _ => fallback,
    }
}

#[cfg(feature = "client")]
fn receive_combat_cues(
    mut commands: Commands,
    mut recent: ResMut<RecentCombatEvents>,
    mut observation: ResMut<ClientCombatObservation>,
    mut capture: Option<ResMut<CaptureCombatCues>>,
    mut receivers: Query<
        Option<&mut lightyear::prelude::MessageReceiver<CombatCue>>,
        With<lightyear::prelude::client::Client>,
    >,
    fighters: Query<(&NetworkEntityId, &Position), With<Fighter>>,
    local_fighter: Query<&PlayerId, (With<Fighter>, With<lightyear::prelude::Controlled>)>,
) {
    let local_player = local_fighter.iter().next().copied();
    for receiver in &mut receivers {
        let Some(mut receiver) = receiver else {
            continue;
        };
        let cues: Vec<_> = receiver.receive().collect();
        for cue in cues {
            match &cue {
                CombatCue::Defeat { .. } | CombatCue::FighterDefeated { .. } => {
                    observation.saw_defeat = true;
                }
                CombatCue::Reset { .. } | CombatCue::FighterReset { .. } => {
                    observation.saw_reset = true;
                }
                _ => {}
            }
            let event_id = match &cue {
                CombatCue::AttackAccepted { event_id, .. }
                | CombatCue::DeliveryImpact { event_id, .. }
                | CombatCue::LobLanded { event_id, .. }
                | CombatCue::MeleeContact { event_id, .. }
                | CombatCue::DamageApplied { event_id, .. }
                | CombatCue::EffectApplied { event_id, .. }
                | CombatCue::FighterDefeated { event_id, .. }
                | CombatCue::FighterReset { event_id, .. }
                | CombatCue::Muzzle { event_id, .. }
                | CombatCue::Impact { event_id, .. }
                | CombatCue::Damage { event_id, .. }
                | CombatCue::Defeat { event_id, .. }
                | CombatCue::Reset { event_id, .. } => *event_id,
            };
            if !remember_combat_event(&mut recent, event_id) {
                continue;
            }
            let profile_id = combat_cue_profile_id(&cue);
            if let Some(capture) = capture.as_mut() {
                if capture.cues.len() < MAX_COMBAT_EVIDENCE_EVENTS {
                    capture.cues.push(cue.clone());
                } else {
                    capture.dropped_cues = capture.dropped_cues.saturating_add(1);
                }
            }
            if observation.ready_file.is_some() {
                if observation.cue_stream.len() < MAX_COMBAT_EVIDENCE_EVENTS {
                    observation.cue_stream.push(cue.clone());
                } else {
                    observation.dropped_cue_stream =
                        observation.dropped_cue_stream.saturating_add(1);
                }
                let timestamp = match &cue {
                    CombatCue::Muzzle { shot_id, .. } => Some(*shot_id),
                    CombatCue::AttackAccepted { attack_id, .. } => Some(ShotId(attack_id.0)),
                    _ => None,
                };
                if let Some(shot_id) = timestamp {
                    if observation.cue_timestamps.len() < MAX_COMBAT_EVIDENCE_EVENTS {
                        observation
                            .cue_timestamps
                            .push((shot_id, unix_epoch_micros()));
                    } else {
                        observation.dropped_cue_timestamps =
                            observation.dropped_cue_timestamps.saturating_add(1);
                    }
                }
            }
            if matches!(
                &cue,
                CombatCue::Muzzle { .. }
                    | CombatCue::Impact { .. }
                    | CombatCue::Damage { .. }
                    | CombatCue::Defeat { .. }
                    | CombatCue::Reset { .. }
            ) {
                continue;
            }
            let target_position = match &cue {
                CombatCue::Damage { target, .. }
                | CombatCue::DamageApplied { target, .. }
                | CombatCue::Defeat { target, .. }
                | CombatCue::FighterDefeated { target, .. }
                | CombatCue::EffectApplied { target, .. }
                | CombatCue::MeleeContact { target, .. } => fighters
                    .iter()
                    .find(|(network_id, _)| **network_id == *target)
                    .map(|(_, position)| position.0),
                CombatCue::AttackAccepted { source, .. } => fighters
                    .iter()
                    .find(|(network_id, _)| **network_id == *source)
                    .map(|(_, position)| position.0),
                _ => None,
            };
            let local_hit = match &cue {
                CombatCue::Damage {
                    source: DamageSource::PlayerWeapon { player_id, .. },
                    ..
                }
                | CombatCue::DamageApplied {
                    source: DamageSource::PlayerWeapon { player_id, .. },
                    ..
                } => local_player == Some(*player_id),
                _ => false,
            };
            let (position, color, size) = match cue {
                CombatCue::AttackAccepted { .. } => (
                    target_position.unwrap_or(Vec2::ZERO),
                    combat_profile_color(profile_id, Color::srgb(1.0, 0.8, 0.2)),
                    combat_profile_size(profile_id, Vec2::splat(16.0)),
                ),
                CombatCue::DeliveryImpact { position, .. }
                | CombatCue::LobLanded { position, .. }
                | CombatCue::MeleeContact { position, .. }
                | CombatCue::Impact { position, .. } => (
                    position.as_vec2(),
                    combat_profile_color(profile_id, Color::srgb(1.0, 0.35, 0.1)),
                    combat_profile_size(profile_id, Vec2::splat(28.0)),
                ),
                CombatCue::DamageApplied { .. } | CombatCue::Damage { .. } => (
                    target_position.unwrap_or(Vec2::ZERO),
                    combat_profile_color(
                        profile_id,
                        if local_hit {
                            Color::srgb(1.0, 0.9, 0.2)
                        } else {
                            Color::srgb(1.0, 0.1, 0.1)
                        },
                    ),
                    combat_profile_size(profile_id, Vec2::splat(18.0)),
                ),
                CombatCue::EffectApplied { .. } => (
                    target_position.unwrap_or(Vec2::ZERO),
                    combat_profile_color(profile_id, Color::srgb(0.3, 0.8, 1.0)),
                    combat_profile_size(profile_id, Vec2::splat(24.0)),
                ),
                CombatCue::FighterDefeated { .. } | CombatCue::Defeat { .. } => (
                    target_position.unwrap_or(Vec2::ZERO),
                    combat_profile_color(profile_id, Color::srgb(0.9, 0.05, 0.05)),
                    combat_profile_size(profile_id, Vec2::splat(64.0)),
                ),
                CombatCue::FighterReset { position, .. } | CombatCue::Reset { position, .. } => (
                    position.as_vec2(),
                    Color::srgb(0.2, 1.0, 0.4),
                    Vec2::splat(42.0),
                ),
                CombatCue::Muzzle { position, .. } => (
                    position.as_vec2(),
                    Color::srgb(1.0, 0.8, 0.2),
                    Vec2::splat(22.0),
                ),
            };
            commands.spawn((
                CombatEffect {
                    timer: Timer::from_seconds(0.18, TimerMode::Once),
                },
                Sprite::from_color(color, size),
                Transform::from_translation(position.extend(30.0)),
            ));
        }
    }
}

#[cfg(feature = "client")]
fn receive_combat_evidence_checkpoints(
    mut observation: ResMut<ClientCombatObservation>,
    mut receivers: Query<
        Option<&mut lightyear::prelude::MessageReceiver<CombatEvidenceCheckpoint>>,
        With<lightyear::prelude::client::Client>,
    >,
) {
    for receiver in &mut receivers {
        let Some(mut receiver) = receiver else {
            continue;
        };
        for checkpoint in receiver.receive() {
            if observation.expected_checkpoints.len() < MAX_COMBAT_EVIDENCE_EVENTS {
                observation.expected_checkpoints.push(checkpoint);
            }
        }
    }
}

#[cfg(feature = "client")]
fn capture_client_combat_checkpoints(
    mut observation: ResMut<ClientCombatObservation>,
    fighters: Query<
        (
            &NetworkEntityId,
            Option<&SelectedBuild>,
            Option<&ResolvedWeapon>,
            Option<&WeaponState>,
            Option<&ActiveEffects>,
            Option<&KnockbackFeedback>,
            Option<&Defeated>,
            Option<&CurrentHealth>,
            Option<&AuthoritativePose>,
            &Position,
            &Rotation,
            Option<&AuthoritativeTick>,
        ),
        With<Fighter>,
    >,
    projectiles: Query<
        (
            &Position,
            Option<&AttackDelivery>,
            Option<&ReplicatedAttackSource>,
            Option<&LobbedFlight>,
            Option<&ProjectileDeadline>,
            Option<&StraightFlight>,
        ),
        With<Projectile>,
    >,
) {
    if observation.ready_file.is_none() {
        return;
    }
    let mut fighter_snapshots = fighters
        .iter()
        .map(
            |(
                network_id,
                selected_build,
                resolved_weapon,
                weapon_state,
                active_effects,
                knockback_feedback,
                defeated,
                health,
                authoritative_pose,
                position,
                rotation,
                authoritative_tick,
            )| CombatFighterSnapshot {
                network_entity_id: *network_id,
                selected_build: selected_build.copied(),
                resolved_weapon: resolved_weapon.cloned(),
                weapon_state: weapon_state.copied(),
                active_effects: active_effects.copied(),
                knockback_feedback: knockback_feedback.copied(),
                defeated: defeated.copied(),
                health: health.map(|health| health.0),
                position: authoritative_pose
                    .map_or_else(|| WorldPoint::from(position.0), |pose| pose.position),
                facing: authoritative_pose
                    .map_or_else(|| rotation.as_radians(), |pose| pose.facing),
                authoritative_tick: authoritative_tick.copied().unwrap_or_default(),
            },
        )
        .collect::<Vec<_>>();
    fighter_snapshots.sort_by_key(|fighter| fighter.network_entity_id.0);
    let authoritative_tick = fighter_snapshots
        .iter()
        .map(|fighter| fighter.authoritative_tick.0)
        .max()
        .unwrap_or(0);
    let mut projectile_snapshots = projectiles
        .iter()
        .filter_map(
            |(position, delivery, source, lobbed_flight, deadline, straight_flight)| {
                CombatProjectileSnapshot::from_components(
                    WorldPoint::from(position.0),
                    delivery,
                    source,
                    lobbed_flight,
                    deadline,
                    straight_flight,
                    authoritative_tick,
                )
            },
        )
        .collect::<Vec<_>>();
    projectile_snapshots
        .sort_by_key(|projectile| (projectile.attack_id.0, projectile.delivery_index));
    let snapshot = CombatStateSnapshot {
        authoritative_tick,
        fighters: fighter_snapshots,
        projectiles: projectile_snapshots,
    };
    observation
        .snapshot_history
        .insert(authoritative_tick, snapshot.clone());
    while observation.snapshot_history.len() > MAX_COMBAT_SNAPSHOT_HISTORY {
        observation.snapshot_history.pop_first();
    }
    let Some(encoded) = encode_state_snapshot(&snapshot) else {
        return;
    };
    if observation.last_encoded_snapshot.as_deref() != Some(encoded.as_str()) {
        if observation.state_mutation_timestamps.len() < MAX_COMBAT_EVIDENCE_EVENTS {
            observation
                .state_mutation_timestamps
                .push((authoritative_tick, unix_epoch_micros()));
        }
        observation.last_encoded_snapshot = Some(encoded);
    }
    let expected_checkpoints = std::mem::take(&mut observation.expected_checkpoints);
    let mut unmatched_checkpoints = Vec::new();
    for expected in expected_checkpoints {
        let checkpoint = expected.checkpoint.as_str();
        if observation
            .checkpoint_matches
            .get(checkpoint)
            .is_some_and(|matches| matches.len() >= 16)
        {
            continue;
        }
        let Some(snapshot) = observation
            .snapshot_history
            .values()
            .chain(std::iter::once(&snapshot))
            .find_map(|candidate| {
                let candidate = if matches!(checkpoint, "defeat" | "reset") {
                    CombatStateSnapshot {
                        projectiles: Vec::new(),
                        ..candidate.clone()
                    }
                } else {
                    candidate.clone()
                };
                (candidate == expected.snapshot).then_some(candidate)
            })
        else {
            if observation.checkpoints.contains_key(checkpoint) {
                continue;
            }
            unmatched_checkpoints.push(expected);
            continue;
        };
        let matches = observation
            .checkpoint_matches
            .entry(checkpoint.to_string())
            .or_default();
        if !matches.iter().any(|candidate| candidate == &snapshot) {
            matches.push(snapshot.clone());
        }
        observation
            .checkpoint_timestamps
            .entry(checkpoint.to_string())
            .or_insert_with(unix_epoch_micros);
        observation
            .checkpoints
            .entry(checkpoint.to_string())
            .or_insert(snapshot);
    }
    observation.expected_checkpoints = unmatched_checkpoints;
}

#[cfg(feature = "client")]
fn record_headless_combat_observation(
    mut observation: ResMut<ClientCombatObservation>,
    config: Res<crate::config::ClientNetworkConfig>,
    automation: Res<crate::client::HeadlessAutomation>,
    fighters: Query<
        (
            &NetworkEntityId,
            &CurrentHealth,
            &WeaponState,
            &FighterDefinitionId,
            Option<&ResolvedWeapon>,
            Option<&Defeated>,
        ),
        With<Fighter>,
    >,
) {
    let Some(path) = observation.ready_file.clone() else {
        return;
    };
    if automation
        .simulation_ticks
        .is_some_and(|limit| automation.elapsed_ticks < limit)
    {
        return;
    }
    if observation.wrote_ready
        || !observation.saw_defeat
        || !observation.saw_reset
        || config.weapon_preset.is_some_and(|preset| {
            required_client_checkpoints(preset)
                .iter()
                .any(|checkpoint| !observation.checkpoints.contains_key(*checkpoint))
        })
    {
        return;
    }
    let Some(_) = fighters
        .iter()
        .filter(|(network_id, _, _, _, _, _)| network_id.0 != DUMMY_NETWORK_ENTITY.0)
        .next()
    else {
        return;
    };
    let mut report = format!(
        "client_elapsed_ms={}\nclient_observation_epoch_us={}\ncue_count={}\nstate_mutation_count={}\npending_checkpoint_count={}\ndropped_cue_stream={}\ndropped_cue_timestamps={}\n",
        observation.started_at.elapsed().as_millis(),
        unix_epoch_micros(),
        observation.cue_stream.len(),
        observation.state_mutation_timestamps.len(),
        observation.expected_checkpoints.len(),
        observation.dropped_cue_stream,
        observation.dropped_cue_timestamps,
    );
    for (checkpoint, snapshot) in &observation.checkpoints {
        if let Some(encoded) = encode_state_snapshot(snapshot) {
            let _ = writeln!(report, "checkpoint_{checkpoint}={encoded}");
            let _ = writeln!(
                report,
                "checkpoint_{checkpoint}_tick={}",
                snapshot.authoritative_tick
            );
        }
        if let Some(timestamp) = observation.checkpoint_timestamps.get(checkpoint) {
            let _ = writeln!(
                report,
                "checkpoint_{checkpoint}_observed_epoch_us={timestamp}"
            );
        }
    }
    for (checkpoint, snapshots) in &observation.checkpoint_matches {
        for snapshot in snapshots {
            if let Some(encoded) = encode_state_snapshot(snapshot) {
                let _ = writeln!(report, "checkpoint_{checkpoint}_candidate={encoded}");
            }
        }
    }
    for (tick, timestamp) in &observation.state_mutation_timestamps {
        let _ = writeln!(report, "state_mutation_tick={tick}_epoch_us={timestamp}");
    }
    for (shot_id, timestamp) in &observation.cue_timestamps {
        let _ = writeln!(report, "cue_shot_id={}_epoch_us={}", shot_id.0, timestamp);
    }
    for cue in &observation.cue_stream {
        let _ = writeln!(report, "cue_stream={}", encode_combat_cue(cue));
    }
    match fs::write(&path, report) {
        Ok(()) => {
            observation.wrote_ready = true;
            info!(path = %path.display(), "headless client observed combat defeat and reset");
        }
        Err(error) => warn!(
            path = %path.display(),
            ?error,
            "headless combat observation write failed"
        ),
    }
}

#[cfg(feature = "client")]
fn required_client_checkpoints(preset_id: u16) -> &'static [&'static str] {
    match preset_id {
        1 => &["defeat", "reset"],
        2 => &["active_scatter_flight", "defeat", "reset"],
        3 => &[
            "active_lob_flight",
            "active_slow",
            "active_knockback",
            "defeat",
            "reset",
        ],
        4 => &["active_knockback", "defeat", "reset"],
        _ => &[],
    }
}

#[cfg(feature = "client")]
fn ensure_projectile_visuals(
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            Option<&Transform>,
            Option<&mut Sprite>,
            Option<&ProjectileSource>,
            Option<&ReplicatedAttackSource>,
            Option<&LobbedFlight>,
        ),
        With<Projectile>,
    >,
) {
    for (entity, transform, sprite, source, replicated_attack, lobbed) in &mut query {
        if transform.is_none() {
            commands.entity(entity).insert(Transform::default());
        }
        let color = source.map_or(Color::srgb(1.0, 0.85, 0.2), |source| {
            projectile_color(source.player_id)
        });
        let profile_id =
            replicated_attack.map_or(1, |source| source.attack.presentation_profile_id.0);
        let size = source.map_or(Vec2::new(20.0, 8.0), |_| match profile_id {
            2 => Vec2::new(9.0, 5.0),
            3 => Vec2::new(16.0, 16.0),
            4 => Vec2::new(24.0, 6.0),
            _ => Vec2::new(20.0, 8.0),
        });
        if let Some(mut sprite) = sprite {
            sprite.color = color;
            sprite.custom_size = Some(size);
        } else {
            commands.entity(entity).insert((
                Sprite::from_color(color, size),
                Name::new(if lobbed.is_some() {
                    "Arc projectile"
                } else {
                    "Weapon delivery"
                }),
            ));
        }
    }
}

#[cfg(feature = "client")]
fn sync_projectile_visuals(
    tick: Query<&AuthoritativeTick>,
    mut query: Query<
        (&Position, &Rotation, &mut Transform, Option<&LobbedFlight>),
        With<Projectile>,
    >,
) {
    let current_tick = tick.iter().next().map_or(0, |tick| tick.0);
    for (position, rotation, mut transform, lobbed) in &mut query {
        transform.translation.x = position.0.x;
        transform.translation.y = position.0.y;
        if let Some(lobbed) = lobbed {
            let progress = (current_tick.saturating_sub(lobbed.launched_at_tick) as f32)
                / (lobbed
                    .lands_at_tick
                    .saturating_sub(lobbed.launched_at_tick)
                    .max(1) as f32);
            transform.translation.z =
                20.0 + delivery::lob_height(progress, lobbed.visual_arc_height);
            transform.rotation = Quat::IDENTITY;
        } else {
            transform.translation.z = 20.0;
            transform.rotation = Quat::from_rotation_z(rotation.as_radians());
        }
    }
}

#[cfg(feature = "client")]
#[derive(Component)]
struct WeaponPreviewVisual {
    slot: u8,
}

#[cfg(feature = "client")]
fn update_weapon_preview(
    mut commands: Commands,
    arena: Res<crate::movement::GreyboxArenaDefinition>,
    fighters: Query<
        (&Position, &Rotation, Option<&ResolvedWeapon>),
        (With<Fighter>, With<lightyear::prelude::Controlled>),
    >,
    mut visuals: Query<(
        &WeaponPreviewVisual,
        &mut Transform,
        &mut Sprite,
        &mut Visibility,
    )>,
) {
    let Some((position, rotation, resolved)) = fighters.iter().next() else {
        for (_, _, _, mut visibility) in &mut visuals {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let Some(resolved) = resolved else {
        for (_, _, _, mut visibility) in &mut visuals {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let origin = position.0;
    let facing = rotation.as_radians();
    let segments = client::preview_segments(origin, facing, resolved, &arena);
    for (visual, mut transform, mut sprite, mut visibility) in &mut visuals {
        let Some((center, angle, size, color)) = segments.get(usize::from(visual.slot)) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Inherited;
        transform.translation = center.extend(11.0);
        transform.rotation = Quat::from_rotation_z(*angle);
        sprite.color = *color;
        sprite.custom_size = Some(*size);
    }
    let existing_slots: HashSet<_> = visuals
        .iter()
        .map(|(visual, _, _, _)| visual.slot)
        .collect();
    for slot in 0..client::MAX_PREVIEW_SEGMENTS as u8 {
        if !existing_slots.contains(&slot) {
            commands.spawn((
                WeaponPreviewVisual { slot },
                Sprite::from_color(Color::srgba(1.0, 1.0, 1.0, 0.0), Vec2::splat(1.0)),
                Transform::default(),
                Visibility::Hidden,
            ));
        }
    }
}

#[cfg(feature = "client")]
fn update_health_bars(
    mut commands: Commands,
    fighters: Query<
        (
            Entity,
            &Position,
            &CurrentHealth,
            &FighterDefinitionId,
            Option<&Defeated>,
        ),
        With<Fighter>,
    >,
    definitions: Res<FighterDefinitions>,
    mut bars: Query<(Entity, &CombatHealthBar, &mut Transform, &mut Sprite)>,
) {
    let fighter_data: HashMap<_, _> = fighters
        .iter()
        .map(|(entity, position, health, definition_id, defeated)| {
            let maximum = definitions
                .get(*definition_id)
                .map_or(0, |definition| definition.maximum_health);
            (entity, (position.0, health.0, maximum, defeated.is_some()))
        })
        .collect();
    let existing: HashSet<_> = bars
        .iter()
        .map(|(_, bar, _, _)| (bar.target, bar.fill))
        .collect();
    for entity in fighter_data.keys().copied() {
        if !existing.contains(&(entity, false)) {
            commands.spawn((
                CombatHealthBar {
                    target: entity,
                    fill: false,
                },
                Sprite::from_color(Color::srgb(0.04, 0.05, 0.07), Vec2::new(56.0, 7.0)),
                Transform::from_xyz(0.0, 0.0, 35.0),
            ));
        }
        if !existing.contains(&(entity, true)) {
            commands.spawn((
                CombatHealthBar {
                    target: entity,
                    fill: true,
                },
                Sprite::from_color(Color::srgb(0.2, 0.95, 0.35), Vec2::new(52.0, 5.0)),
                Transform::from_xyz(0.0, 0.0, 36.0),
            ));
        }
    }
    for (bar_entity, bar, mut transform, mut sprite) in &mut bars {
        let Some((position, health, maximum, defeated)) = fighter_data.get(&bar.target) else {
            commands.entity(bar_entity).despawn();
            continue;
        };
        let ratio = f32::from(*health) / f32::from((*maximum).max(1));
        transform.translation.x = position.x;
        transform.translation.y = position.y + 34.0;
        if bar.fill {
            transform.translation.x -= 26.0 * (1.0 - ratio);
            transform.scale.x = ratio;
            sprite.color = if *defeated {
                Color::srgb(0.75, 0.08, 0.08)
            } else {
                Color::srgb(0.2, 0.95, 0.35)
            };
        } else {
            transform.scale.x = 1.0;
        }
    }
}

#[cfg(feature = "client")]
fn update_combat_hud(
    mut text: Query<&mut Text, With<CombatHudText>>,
    fighter: Query<
        (
            &PlayerId,
            &CurrentHealth,
            &WeaponState,
            Option<&AuthoritativeTick>,
            Option<&SelectedBuild>,
            Option<&ResolvedWeapon>,
            Option<&ActiveEffects>,
            Option<&Defeated>,
        ),
        (With<Fighter>, With<lightyear::prelude::Controlled>),
    >,
    weapons: Res<WeaponDefinitions>,
    catalog: Option<Res<WeaponCatalogResource>>,
) {
    let Some((
        player_id,
        health,
        state,
        authoritative_tick,
        build,
        resolved,
        active_effects,
        defeated,
    )) = fighter.iter().next()
    else {
        return;
    };
    let weapon_id = build.map_or(PULSE_SIDEARM_DEFINITION, |build| build.primary_weapon);
    let capacity = resolved.map_or_else(
        || {
            weapons
                .get(weapon_id)
                .map_or(0, |weapon| weapon.magazine_capacity)
        },
        |resolved| resolved.recipe.economy.capacity(),
    );
    let weapon_name = resolved
        .and_then(|resolved| resolved.source_preset_id)
        .and_then(|id| catalog.as_ref().and_then(|catalog| catalog.0.preset(id)))
        .map_or_else(
            || {
                if weapon_id == PULSE_SIDEARM_DEFINITION {
                    "Pulse"
                } else {
                    "Weapon"
                }
            },
            |preset| preset.display_name.as_str(),
        );
    let phase = match state.phase {
        WeaponPhase::Ready => "READY".to_string(),
        WeaponPhase::Cooldown { ready_at_tick } | WeaponPhase::Reloading { ready_at_tick }
            if authoritative_tick.is_some() =>
        {
            let label = if matches!(state.phase, WeaponPhase::Cooldown { .. }) {
                "COOLDOWN"
            } else {
                "RELOADING"
            };
            format!(
                "{label} {}t",
                ready_at_tick.saturating_sub(authoritative_tick.expect("checked above").0)
            )
        }
        WeaponPhase::Cooldown { .. } | WeaponPhase::Reloading { .. } => "SYNCING".to_string(),
    };
    let phase = defeated.map_or(phase, |_| "DEFEATED".to_string());
    let slow = active_effects
        .and_then(|effects| effects.slow)
        .zip(authoritative_tick)
        .filter(|(slow, tick)| slow.expires_at_tick > tick.0)
        .map_or_else(String::new, |(slow, tick)| {
            format!("  SLOW {}t", slow.expires_at_tick.saturating_sub(tick.0))
        });
    for mut value in &mut text {
        **value = format!(
            "Player {}   Health {:>3}   {} {}/{}   {}{}",
            player_id.0, health.0, weapon_name, state.ammo, capacity, phase, slow
        );
    }
}

#[cfg(feature = "client")]
fn update_durable_effect_markers(
    mut commands: Commands,
    fighters: Query<
        (
            Entity,
            &Position,
            Option<&AuthoritativeTick>,
            Option<&ActiveEffects>,
            Option<&KnockbackFeedback>,
            Option<&Defeated>,
        ),
        With<Fighter>,
    >,
    mut markers: Query<(Entity, &CombatStatusMarker, &mut Transform, &mut Sprite)>,
) {
    let desired: HashMap<_, _> = fighters
        .iter()
        .flat_map(
            |(entity, position, authoritative_tick, active_effects, knockback, defeated)| {
                if defeated.is_some() {
                    return Vec::new();
                }
                let mut markers = Vec::with_capacity(2);
                if active_effects.is_some_and(|effects| {
                    effects.slow.is_some_and(|slow| {
                        authoritative_tick.is_none_or(|tick| tick.0 < slow.expires_at_tick)
                    })
                }) {
                    markers.push((
                        CombatStatusMarker {
                            target: entity,
                            kind: CombatStatusKind::Slow,
                        },
                        (position.0, Color::srgba(0.25, 0.75, 1.0, 0.85)),
                    ));
                }
                if knockback.is_some() {
                    markers.push((
                        CombatStatusMarker {
                            target: entity,
                            kind: CombatStatusKind::Knockback,
                        },
                        (position.0, Color::srgba(1.0, 0.55, 0.18, 0.85)),
                    ));
                }
                markers
            },
        )
        .collect();
    let mut existing = HashSet::new();
    for (marker_entity, marker, mut transform, mut sprite) in &mut markers {
        if let Some((position, color)) = desired.get(marker) {
            existing.insert(*marker);
            transform.translation = position.extend(39.0);
            sprite.color = *color;
        } else {
            commands.entity(marker_entity).despawn();
        }
    }
    for (marker, (position, color)) in desired {
        if !existing.contains(&marker) {
            commands.spawn((
                marker,
                Sprite::from_color(color, Vec2::splat(13.0)),
                Transform::from_translation(position.extend(39.0)),
            ));
        }
    }
}

#[cfg(feature = "client")]
fn update_combat_effects(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut effects: Query<(Entity, &mut CombatEffect)>,
) {
    for (entity, mut effect) in &mut effects {
        effect.timer.tick(time.delta());
        if effect.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "client")]
    use crate::timing::SimulationTick;

    #[cfg(feature = "client")]
    use core::time::Duration;

    #[test]
    fn authored_catalogs_validate_and_have_expected_values() {
        let fighters = FighterDefinitions::default();
        let weapons = WeaponDefinitions::default();
        assert!(fighters.validate(&weapons).is_ok());
        assert!(weapons.validate(&fighters).is_ok());
        assert_eq!(
            fighters
                .get(STANDARD_FIGHTER_DEFINITION)
                .unwrap()
                .maximum_health,
            100
        );
        assert_eq!(
            weapons
                .get(PULSE_SIDEARM_DEFINITION)
                .unwrap()
                .magazine_capacity,
            6
        );
    }

    #[test]
    fn catalog_validation_rejects_duplicate_and_unsafe_values() {
        let mut fighters = FighterDefinitions::default();
        fighters.entries.push(fighters.entries[0]);
        assert!(fighters.validate(&WeaponDefinitions::default()).is_err());
        fighters.entries.pop();
        fighters.entries[0].movement_speed = f32::NAN;
        assert!(fighters.validate(&WeaponDefinitions::default()).is_err());
        assert!(
            FighterDefinitions {
                entries: Vec::new()
            }
            .validate(&WeaponDefinitions::default())
            .is_err()
        );
        let mut weapons = WeaponDefinitions::default();
        weapons.entries[0].muzzle_offset = 1.0;
        assert!(weapons.validate(&FighterDefinitions::default()).is_err());
        weapons.entries[0].maximum_range = 0.0;
        assert!(weapons.validate(&FighterDefinitions::default()).is_err());
        assert!(
            WeaponDefinitions {
                entries: Vec::new()
            }
            .validate(&FighterDefinitions::default())
            .is_err()
        );
    }

    #[test]
    fn combat_cue_evidence_encoding_round_trips_full_payload() {
        let cue = CombatCue::Impact {
            event_id: CombatEventId(7),
            tick: 42,
            source: NetworkEntityId(11),
            shot_id: ShotId(13),
            weapon_definition_id: PULSE_SIDEARM_DEFINITION,
            target: Some(NetworkEntityId(17)),
            position: WorldPoint { x: 1.5, y: -2.5 },
            normal: WorldPoint { x: -1.0, y: 0.0 },
            distance_band: DistanceBand::Mid,
        };

        let encoded = encode_combat_cue(&cue);
        assert_eq!(decode_combat_cue(&encoded), Some(cue));
        assert!(decode_combat_cue("abc").is_none());
    }

    #[cfg(feature = "server")]
    #[test]
    fn fire_economy_boundaries_are_integer_and_deterministic() {
        let fighters = FighterDefinitions::default();
        let fighter = fighters
            .get(STANDARD_FIGHTER_DEFINITION)
            .expect("standard fighter definition");
        let recipe = WeaponCatalog::embedded()
            .expect("embedded catalog")
            .resolve_preset(WeaponPresetId(1), fighter)
            .expect("pulse preset")
            .recipe;
        let mut state = WeaponState {
            ammo: 1,
            phase: WeaponPhase::Ready,
        };
        state.ammo -= 1;
        state.phase = WeaponPhase::Reloading { ready_at_tick: 61 };
        assert_eq!(state.ammo, 0);
        advance_composed_weapon_state(&mut state, &recipe, 60);
        assert_eq!(state.ammo, 0);
        assert_eq!(state.phase, WeaponPhase::Reloading { ready_at_tick: 61 });
        advance_composed_weapon_state(&mut state, &recipe, 61);
        assert_eq!(state.ammo, recipe.economy.capacity());
        assert_eq!(state.phase, WeaponPhase::Ready);
        state.phase = WeaponPhase::Cooldown { ready_at_tick: 73 };
        advance_composed_weapon_state(&mut state, &recipe, 72);
        assert_eq!(state.phase, WeaponPhase::Cooldown { ready_at_tick: 73 });
        advance_composed_weapon_state(&mut state, &recipe, 73);
        assert_eq!(state.phase, WeaponPhase::Ready);
    }

    #[cfg(feature = "server")]
    #[test]
    fn fighter_runtime_reads_health_and_ammo_from_selected_definitions() {
        let mut fighters = FighterDefinitions::default();
        fighters.entries[0].maximum_health = 77;
        let mut weapons = WeaponDefinitions::default();
        weapons.entries[0].magazine_capacity = 3;

        let (_, _, _, health, weapon) = default_fighter_runtime(TeamId(4), &fighters, &weapons);

        assert_eq!(health, CurrentHealth(77));
        assert_eq!(weapon.ammo, 3);
    }

    #[test]
    fn allocators_are_monotonic_and_reject_exhaustion() {
        let mut ids = NextCombatIds::default();
        assert_eq!(ids.allocate_shot(), Some(ShotId(1)));
        assert_eq!(ids.allocate_event(), Some(CombatEventId(1)));
        ids.next_shot_id = u64::MAX;
        assert_eq!(ids.allocate_shot(), None);
        assert_eq!(ids.next_shot_id, u64::MAX);
    }

    #[test]
    fn lethal_event_pair_reservation_is_atomic_at_exhaustion() {
        let mut ids = NextCombatIds {
            next_attack_id: 1,
            next_shot_id: 1,
            next_event_id: u64::MAX - 1,
        };
        assert_eq!(ids.allocate_event_pair(), None);
        assert_eq!(ids.next_event_id, u64::MAX - 1);
        assert_eq!(ids.allocate_event(), Some(CombatEventId(u64::MAX - 1)));
    }

    #[test]
    fn neutral_entities_are_hostile_to_every_team() {
        assert!(teams_are_hostile(NEUTRAL_TEAM, NEUTRAL_TEAM));
        assert!(teams_are_hostile(NEUTRAL_TEAM, TeamId(0)));
        assert!(teams_are_hostile(TeamId(0), NEUTRAL_TEAM));
        assert!(teams_are_hostile(TeamId(0), TeamId(1)));
        assert!(!teams_are_hostile(TeamId(0), TeamId(0)));
    }

    #[test]
    fn diagnostic_record_history_is_bounded() {
        let mut telemetry = CombatTelemetry::default();
        for index in 0..(MAX_COMBAT_RECORDS + 32) {
            telemetry.record(CombatLogRecord::Shot {
                event_id: CombatEventId(index as u64 + 1),
                tick: index as u64,
                shot_id: ShotId(index as u64 + 1),
                source: NetworkEntityId(1),
                weapon: PULSE_SIDEARM_DEFINITION,
                muzzle_position: WorldPoint { x: 0.0, y: 0.0 },
                ammo_after: 5,
            });
        }
        assert_eq!(telemetry.records.len(), MAX_COMBAT_RECORDS);
    }

    #[test]
    fn distance_bands_follow_the_authored_boundaries() {
        assert_eq!(distance_band(249.9), DistanceBand::Close);
        assert_eq!(distance_band(250.0), DistanceBand::Mid);
        assert_eq!(distance_band(599.9), DistanceBand::Mid);
        assert_eq!(distance_band(600.0), DistanceBand::Long);
    }

    #[cfg(feature = "server")]
    #[test]
    fn muzzle_position_is_finite_and_follows_authoritative_facing() {
        let position = muzzle_position(Vec2::new(10.0, -5.0), 0.0, 34.0);
        assert_eq!(position, Vec2::new(44.0, -5.0));
        assert!(muzzle_position(Vec2::ZERO, std::f32::consts::FRAC_PI_2, 34.0).is_finite());
    }

    #[cfg(feature = "server")]
    #[test]
    fn reset_deadline_is_inactive_before_and_active_at_the_deadline() {
        assert!(!reset_is_due(89, 90));
        assert!(reset_is_due(90, 90));
    }

    #[cfg(feature = "client")]
    #[test]
    fn combat_cue_event_ids_are_deduplicated_with_a_bounded_history() {
        let mut recent = RecentCombatEvents::default();
        assert!(remember_combat_event(&mut recent, CombatEventId(1)));
        assert!(!remember_combat_event(&mut recent, CombatEventId(1)));
        for event_id in 2..=257 {
            assert!(remember_combat_event(&mut recent, CombatEventId(event_id)));
        }
        assert_eq!(recent.ids.len(), 256);
        assert!(!recent.ids.contains(&CombatEventId(1)));
        assert!(remember_combat_event(&mut recent, CombatEventId(1)));
    }

    #[cfg(feature = "client")]
    #[test]
    fn combat_effects_expire_after_the_bounded_presentation_lifetime() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_millis(100),
            ))
            .add_systems(Update, update_combat_effects);
        let effect = app
            .world_mut()
            .spawn(CombatEffect {
                timer: Timer::from_seconds(0.18, TimerMode::Once),
            })
            .id();

        app.update();
        assert!(app.world().get_entity(effect).is_ok());
        app.update();
        app.update();

        assert!(app.world().get_entity(effect).is_err());
    }

    #[cfg(feature = "client")]
    #[test]
    fn combat_hud_reports_replicated_reload_and_defeat_state() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<WeaponDefinitions>()
            .insert_resource(SimulationTick(999))
            .add_systems(Update, update_combat_hud);
        let hud = app
            .world_mut()
            .spawn((CombatHudText, Text::new("placeholder")))
            .id();
        app.world_mut().spawn((
            Fighter,
            lightyear::prelude::Controlled,
            PlayerId(1),
            CurrentHealth(42),
            AuthoritativeTick(10),
            WeaponState {
                ammo: 0,
                phase: WeaponPhase::Reloading { ready_at_tick: 25 },
            },
        ));

        app.update();
        assert_eq!(
            app.world().get::<Text>(hud).expect("combat HUD").0,
            "Player 1   Health  42   Pulse 0/6   RELOADING 15t"
        );

        app.world_mut().entity_mut(hud).insert(Text::new("stale"));
        let fighter = app
            .world_mut()
            .query_filtered::<Entity, With<Fighter>>()
            .single(app.world())
            .expect("controlled fighter");
        app.world_mut().entity_mut(fighter).insert(Defeated {
            event_id: CombatEventId(1),
            reset_at_tick: 100,
        });
        app.update();
        assert_eq!(
            app.world().get::<Text>(hud).expect("combat HUD").0,
            "Player 1   Health  42   Pulse 0/6   DEFEATED"
        );
    }

    #[cfg(feature = "client")]
    #[test]
    fn fighter_and_projectile_palettes_distinguish_replicated_sources() {
        assert_ne!(fighter_color(PlayerId(1)), fighter_color(PlayerId(2)));
        assert_ne!(projectile_color(PlayerId(1)), projectile_color(PlayerId(2)));
        assert_ne!(fighter_color(PlayerId(1)), Color::srgb(0.95, 0.25, 0.1));
    }

    #[cfg(feature = "client")]
    #[test]
    fn projectile_presentation_keeps_authoritative_position_and_facing() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Update, sync_projectile_visuals);
        let projectile = app
            .world_mut()
            .spawn((
                Projectile,
                Position::from_xy(120.0, -40.0),
                Rotation::radians(std::f32::consts::FRAC_PI_2),
                Transform::default(),
            ))
            .id();

        app.update();

        let transform = app
            .world()
            .get::<Transform>(projectile)
            .expect("projectile transform");
        assert_eq!(transform.translation.truncate(), Vec2::new(120.0, -40.0));
        assert!(
            (transform.rotation.to_euler(EulerRot::ZYX).0 - std::f32::consts::FRAC_PI_2).abs()
                < 0.001
        );
    }
}

//! Shared combat identities, replicated state, and server-internal delivery messages.

#[cfg(feature = "server")]
use super::{PayloadBundleDefinition, WeaponRecipe, WorldEffectDefinition};
use super::{
    PayloadEffectDefinition, WeaponDefinitionId, WeaponPresentationProfileId, WeaponPresetId,
    WeaponRecipeFingerprint,
};
use crate::protocol::{NetworkEntityId, PlayerId};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
#[cfg(feature = "server")]
use std::collections::BTreeMap;
/// Stable source identity for a player action. Delivery entities and payloads refer to this
/// identity rather than a process-local ECS entity or a preset-specific behavior class.
#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd, Reflect,
)]
pub struct AttackId(pub u64);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct AttackSource {
    pub kind: CombatSourceKind,
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

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatSourceKind {
    PrimaryWeapon,
    Environment,
    Ultimate {
        ultimate_id: crate::builds::UltimateDefinitionId,
    },
    Deployable {
        ultimate_id: crate::builds::UltimateDefinitionId,
        deployable_id: crate::builds::DeployableId,
    },
}

/// Replicated attack identity carried by composed delivery entities. The server remains the
/// authority for the private runtime recipe; clients use this bounded identity for presentation
/// and diagnostics only.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct ReplicatedAttackSource {
    pub attack: AttackSource,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlowEffect {
    pub source_attack_id: AttackId,
    pub source_network_entity_id: NetworkEntityId,
    pub movement_multiplier_milli: u16,
    pub expires_at_tick: u64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum DamageOverTimeKind {
    Poison,
    Fire,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConditionSource {
    pub action_id: AttackId,
    pub kind: CombatSourceKind,
    pub player_id: PlayerId,
    pub network_entity_id: NetworkEntityId,
    pub team_id: TeamId,
    pub source_preset_id: Option<WeaponPresetId>,
    pub recipe_fingerprint: Option<WeaponRecipeFingerprint>,
    pub presentation_profile_id: Option<WeaponPresentationProfileId>,
}

impl From<AttackSource> for ConditionSource {
    fn from(source: AttackSource) -> Self {
        Self {
            action_id: source.attack_id,
            kind: source.kind,
            player_id: source.player_id,
            network_entity_id: source.owner_network_entity_id,
            team_id: source.team_id,
            source_preset_id: source.source_preset_id,
            recipe_fingerprint: Some(source.recipe_fingerprint),
            presentation_profile_id: Some(source.presentation_profile_id),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DamageOverTime {
    pub source: ConditionSource,
    pub damage_per_tick: u16,
    pub tick_interval: u64,
    pub next_tick: u64,
    pub expires_at_tick: u64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ColdState {
    pub meter: u16,
    pub last_contribution_tick: u64,
    pub frozen_until_tick: Option<u64>,
    pub immunity_until_tick: Option<u64>,
    pub source: Option<ConditionSource>,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActiveEffects {
    pub slow: Option<SlowEffect>,
    pub cold: ColdState,
    pub poison: Option<DamageOverTime>,
    pub fire: Option<DamageOverTime>,
}

impl ActiveEffects {
    #[must_use]
    pub fn is_frozen(self, tick: u64) -> bool {
        self.cold
            .frozen_until_tick
            .is_some_and(|deadline| tick < deadline)
    }

    #[must_use]
    pub fn is_poisoned(self, tick: u64) -> bool {
        self.poison
            .is_some_and(|condition| tick <= condition.expires_at_tick)
    }
}

#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd, Reflect,
)]
pub struct ElementalFieldId(pub u64);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum ElementalFieldKind {
    Cryogenic,
    Fire,
    Poison,
    Restoration,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct ElementalFieldState {
    pub id: ElementalFieldId,
    pub kind: ElementalFieldKind,
    pub owner_network_entity_id: NetworkEntityId,
    pub team_id: TeamId,
    pub center: WorldPoint,
    pub radius_milliunits: u32,
    pub activated_at_tick: u64,
    pub next_pulse_tick: u64,
    pub expires_at_tick: u64,
}

impl ElementalFieldState {
    #[must_use]
    pub fn center_vec2(self) -> Vec2 {
        self.center.as_vec2()
    }

    #[must_use]
    pub fn radius(self) -> Option<f32> {
        crate::builds::world_units_from_milliunits(self.radius_milliunits)
    }
}

#[cfg(feature = "server")]
#[derive(Component, Clone, Debug, PartialEq)]
pub struct ElementalFieldRuntime {
    pub source: ConditionSource,
    pub match_id: crate::matchplay::MatchId,
    pub pulse_interval_ticks: u64,
    pub effect: PayloadEffectDefinition,
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

/// Marker for one replicated, stationary cone spray. It is a timed gameplay volume, not a
/// projectile: its origin and facing never follow the fighter after attack acceptance.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConeSpray;

/// Immutable network-visible propagation facts for one accepted perfume-like spritz.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct ConeSprayState {
    pub origin: WorldPoint,
    pub facing: f32,
    pub propagation_speed: f32,
    pub maximum_reach: f32,
    pub angle_degrees: f32,
    pub emitted_at_tick: u64,
    pub full_at_tick: u64,
    pub expires_at_tick: u64,
    pub pulse_interval_ticks: u64,
    pub map_occlusion: bool,
    pub max_targets: u8,
}

/// Marker for one replicated stationary Splash area.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PersistentSplash;

/// Immutable public facts required to render and recover one authoritative Splash area.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct PersistentSplashState {
    pub center: WorldPoint,
    pub facing: f32,
    pub shape: super::PersistentAreaShape,
    pub activated_at_tick: u64,
    pub next_pulse_tick: u64,
    pub expires_at_tick: u64,
    pub pulse_interval_ticks: u64,
    pub map_occlusion: bool,
    pub max_targets: u8,
    pub effects: [Option<PayloadEffectDefinition>; 2],
}

impl ConeSprayState {
    #[must_use]
    pub fn reached_distance(self, tick: u64) -> f32 {
        let elapsed = tick.saturating_sub(self.emitted_at_tick) as f32;
        (elapsed * self.propagation_speed / crate::timing::SIMULATION_TICK_HZ as f32)
            .clamp(0.0, self.maximum_reach)
    }

    #[must_use]
    pub const fn active_at(self, tick: u64) -> bool {
        tick >= self.emitted_at_tick && tick <= self.expires_at_tick
    }
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

/// Public source role for an armed Sticky Blomb. The role is part of the chain-detonation rule:
/// only a new primary impact may detonate an already attached primary.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum StickyBlobKind {
    Primary,
    UltimateSecondary,
}

/// Replicated, bounded state for one armed delayed explosion.
///
/// `Position` carries the current authoritative center. When `attached_to` is present the server
/// updates that center from the carrier every fixed tick; clients use the radius and deadline to
/// draw the mandatory moving future-blast telegraph.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct StickyBlobState {
    pub kind: StickyBlobKind,
    pub attached_to: Option<NetworkEntityId>,
    pub armed_at_tick: u64,
    pub detonates_at_tick: u64,
    pub explosion_radius: f32,
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

/// Authoritative planar collision geometry for a projectile delivery.
///
/// Geometry is deliberately independent from trajectory: [`StraightFlight`] describes how the
/// body moves, while this shape is the single fact used by collision, replication, presentation,
/// and local aim tracing.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Reflect)]
pub enum ProjectileShape {
    Circle { radius: f32 },
}

impl ProjectileShape {
    #[must_use]
    pub const fn circle(radius: f32) -> Self {
        Self::Circle { radius }
    }

    #[must_use]
    pub const fn bounding_radius(self) -> f32 {
        match self {
            Self::Circle { radius } => radius,
        }
    }

    #[must_use]
    pub fn is_valid(self) -> bool {
        let radius = self.bounding_radius();
        radius.is_finite() && radius > 0.0
    }

    #[must_use]
    pub fn collider(self) -> avian2d::prelude::Collider {
        match self {
            Self::Circle { radius } => avian2d::prelude::Collider::circle(radius),
        }
    }
}

/// Replicated immutable projectile body. Straight deliveries always carry this component;
/// non-projectile delivery geometry remains owned by its corresponding delivery component.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct ProjectileBody {
    pub shape: ProjectileShape,
}

impl ProjectileBody {
    #[must_use]
    pub const fn circle(radius: f32) -> Self {
        Self {
            shape: ProjectileShape::circle(radius),
        }
    }

    #[must_use]
    pub fn collider(self) -> avian2d::prelude::Collider {
        self.shape.collider()
    }
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
#[derive(
    Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Reflect,
)]
pub struct TeamId(pub u8);

/// Integer authoritative health.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub struct CurrentHealth(pub u16);

/// Server-only fixed-point health-recovery progress and attack-idle origin.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HealthRecoveryState {
    pub last_accepted_attack_tick: u64,
    /// Numerator in health-points-per-second ticks; always less than the simulation frequency.
    pub recovery_remainder: u64,
}

impl HealthRecoveryState {
    #[must_use]
    pub const fn starting_at(tick: u64) -> Self {
        Self {
            last_accepted_attack_tick: tick,
            recovery_remainder: 0,
        }
    }
}

/// Fire gating only. Ammunition recovery advances independently in [`WeaponState`].
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum WeaponPhase {
    Ready,
    Cooldown { ready_at_tick: u64 },
}

/// One server-authored ammunition recovery interval.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub struct AmmoRecovery {
    pub started_at_tick: u64,
    pub ready_at_tick: u64,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub struct WeaponState {
    pub ammo: u8,
    pub phase: WeaponPhase,
    pub ammo_recovery: Option<AmmoRecovery>,
}

impl WeaponState {
    #[must_use]
    pub const fn ready(ammo: u8) -> Self {
        Self {
            ammo,
            phase: WeaponPhase::Ready,
            ammo_recovery: None,
        }
    }

    #[must_use]
    pub const fn can_fire(self, tick: u64) -> bool {
        self.ammo > 0
            && match self.phase {
                WeaponPhase::Ready => true,
                WeaponPhase::Cooldown { ready_at_tick } => tick >= ready_at_tick,
            }
    }
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
    /// Fighter whose connection/lifecycle owns the attack.
    pub owner_entity: Entity,
    /// Physical entity that emitted the delivery and must be excluded from its collision sweep.
    pub source_entity: Entity,
    pub source: AttackSource,
    pub delivery_index: u8,
    pub velocity: Vec2,
    pub travelled: f32,
    pub expires_at_tick: u64,
    pub maximum_range: f32,
    pub landing: Option<Vec2>,
    pub recipe: WeaponRecipe,
}

#[cfg(feature = "server")]
#[derive(Component, Clone, Debug, PartialEq)]
pub struct StickyBlobRuntime {
    pub source: AttackSource,
    pub delivery_index: u8,
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
#[derive(Component, Clone, Debug, PartialEq)]
pub struct ConeSprayRuntime {
    pub owner_entity: Entity,
    pub source: AttackSource,
    pub recipe: WeaponRecipe,
    pub next_pulse_tick: u64,
    pub next_delivery_index: u8,
    pub match_id: Option<crate::matchplay::MatchId>,
}

#[cfg(feature = "server")]
#[derive(Component, Clone, Debug, PartialEq)]
pub struct PersistentSplashRuntime {
    pub source: AttackSource,
    pub recipe: WeaponRecipe,
    pub next_delivery_index: u8,
    pub match_id: Option<crate::matchplay::MatchId>,
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
    /// Authored delivery-level world effects, copied from the firing recipe. Bounded by
    /// validation to one entry in v1.
    pub world_effects: Vec<WorldEffectDefinition>,
}

/// One committed delivery-level world effect. Combat emits these after the delivery
/// transaction commits; the map authority owns whether any cell actually changes.
#[cfg(feature = "server")]
#[derive(Clone, Debug, PartialEq)]
pub struct CombatWorldEffectFact {
    pub tick: u64,
    pub source: CombatWorldEffectSource,
    pub position: WorldPoint,
    pub effect: WorldEffectDefinition,
}

/// Stable provenance for a committed world effect without fabricating a weapon attack.
#[cfg(feature = "server")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CombatWorldEffectSource {
    Weapon {
        attack: AttackSource,
        delivery_index: u8,
        effect_index: u8,
    },
    Ultimate {
        event_id: CombatEventId,
        owner_network_entity_id: NetworkEntityId,
        ultimate_id: crate::builds::UltimateDefinitionId,
    },
}

/// Bounded ordered world-effect facts for one fixed post-update tick.
#[cfg(feature = "server")]
#[derive(Resource, Default, Debug)]
pub struct CombatWorldEffectFacts(pub Vec<CombatWorldEffectFact>);

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
    StickyDetonated {
        position: WorldPoint,
    },
    MeleeContact {
        target: NetworkEntityId,
        position: WorldPoint,
    },
    ConeSprayPulse {
        origin: WorldPoint,
        facing: f32,
        reached_distance: f32,
        angle_degrees: f32,
    },
    SplashPulse {
        center: WorldPoint,
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

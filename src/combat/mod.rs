//! The first authoritative combat slice: one direct-fire weapon, projectiles, damage, and reset.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "combat math quantizes float weapon/effect values into the bounded u16/u8 wire forms at single, reviewed sites"
)]

pub(crate) mod attack;
mod authority;
#[cfg(feature = "client")]
pub(crate) mod client;
#[cfg(feature = "server")]
pub(crate) mod conditions;
pub(crate) mod cues;
pub(crate) mod definitions;
pub(crate) mod delivery;
#[cfg(feature = "server")]
pub(crate) mod effects;
#[cfg(feature = "server")]
pub(crate) mod environment;
pub(crate) mod evidence;
#[cfg(feature = "server")]
pub(crate) mod fields;
pub(crate) mod model;
pub(crate) mod outcomes;
#[cfg(feature = "server")]
mod recovery;
mod rules;
#[cfg(feature = "server")]
pub(crate) mod server;
#[cfg(feature = "server")]
mod splash;
#[cfg(feature = "server")]
mod spray;
#[cfg(feature = "server")]
pub(crate) mod sticky;
pub(crate) mod telemetry;
#[cfg(feature = "server")]
pub(crate) use telemetry::AbilityWeaponTelemetry;

#[allow(clippy::wildcard_imports)]
#[cfg(feature = "server")]
use authority::*;
#[cfg(feature = "server")]
pub(crate) use authority::{TestDummy, TestDummyFixture, TestDummyResetDeadline};

#[cfg(all(test, feature = "server"))]
use attack::advance_composed_weapon_state;
#[cfg(feature = "server")]
use attack::authoritative_composed_fire;
#[cfg(feature = "server")]
use delivery::{resolve_melee_attacks, sweep_composed_projectiles};
#[cfg(feature = "server")]
use effects::{
    finish_attack_delivery, flush_completed_attack_telemetry, payload_target_visible,
    resolve_composed_payloads,
};
#[cfg(feature = "client")]
pub(crate) use evidence::{
    capture_client_combat_checkpoints, receive_combat_evidence_checkpoints,
    record_headless_combat_observation,
};
#[cfg(feature = "server")]
use evidence::{capture_server_combat_checkpoints, send_combat_evidence_checkpoints};
#[cfg(feature = "server")]
use recovery::restore_attack_idle_health;

pub use crate::content::GameplayContentFingerprint;
#[cfg(feature = "client")]
pub use client::ClientCombatEvidenceStatus;
#[cfg(feature = "client")]
pub(crate) use client::DeduplicatedCombatCue;
#[cfg(feature = "client")]
pub use client::{ClientCombatPlugin, CombatAbilityHudText, CombatHudText};
pub use cues::{
    CombatCue, CombatCueKey, CombatCueKind, CombatEffectCue, DamageSource, SelfCloakEndReason,
    combat_cue_key, decode_combat_cue, encode_combat_cue,
};
pub(crate) use definitions::MAX_WEAPON_PRESETS;
pub use definitions::{
    DamageFalloff, DeliveryMethod, EngineWeaponLimits, FiringPattern, PayloadBundleDefinition,
    PayloadEffectDefinition, PersistentAreaShape, RecipientPolicy, ResolvedWeapon, SlowStacking,
    TargetSelection, WeaponCatalog, WeaponCatalogResource, WeaponConfiguration, WeaponEconomy,
    WeaponPresetDefinition, WeaponPresetId, WeaponRecipe, WeaponRecipeFingerprint,
    WeaponRecipePolicy, WorldEffectDefinition, WorldEffectKind, linear_falloff,
    resolve_configuration, resolve_configuration_with_policy, spread_angles,
};
pub use evidence::{
    CombatCheckpoint, CombatConeSpraySnapshot, CombatEvidenceCheckpoint, CombatFighterSnapshot,
    CombatProjectileSnapshot, CombatStateSnapshot, encode_state_snapshot,
};
#[cfg(feature = "server")]
pub use evidence::{CombatEvidenceSnapshots, CombatOutbox};
#[cfg(feature = "server")]
pub(crate) use model::ElementalFieldRuntime;
#[cfg(feature = "server")]
pub use model::{
    ActiveAttackTracker, ActiveAttackTrackers, CombatWorldEffectFact, CombatWorldEffectFacts,
    CombatWorldEffectSource, CompletedAttack, ComposedProjectileRuntime, ConeSprayRuntime,
    MeleeAttack, PendingDelivery, PendingDeliveryKind, PendingPayload, PersistentSplashRuntime,
    SpawnState, StickyBlobRuntime,
};
pub use model::{
    ActiveEffects, AmmoRecovery, AttackDelivery, AttackId, AttackSource, AuthoritativePose,
    AuthoritativeTick, ColdState, CombatEventId, CombatSourceKind, ConditionSource, ConeSpray,
    ConeSprayState, CurrentHealth, DamageOverTime, DamageOverTimeKind, Defeated, DistanceBand,
    ElementalFieldId, ElementalFieldKind, ElementalFieldState, ExternalMotion, HealthRecoveryState,
    KnockbackFeedback, LobbedFlight, PersistentSplash, PersistentSplashState, Projectile,
    ProjectileBody, ProjectileDeadline, ProjectileShape, ProjectileSource, ReplicatedAttackSource,
    ShotId, SlowEffect, StickyBlobKind, StickyBlobState, StraightFlight, TeamId, WeaponPhase,
    WeaponState, WorldPoint, distance_band,
};
pub use outcomes::{
    AcceptedAttackFact, AcceptedAttackFacts, CombatOutcomeFact, CombatOutcomeFacts,
    CombatOutcomeKind, CombatTargetKind, MAX_ACCEPTED_ATTACK_FACTS_PER_TICK,
};
pub use rules::{
    COMBAT_CONDITION_RULES_SCHEMA_VERSION, CombatConditionRules, CombatConditionRulesResource,
    MAX_COLD_DECAY_PER_TICK, MAX_COLD_RULE_TICKS,
};
#[cfg(feature = "server")]
pub use server::ServerCombatPlugin;
use telemetry::MAX_COMBAT_EVIDENCE_EVENTS;
#[cfg(test)]
use telemetry::MAX_COMBAT_RECORDS;
pub use telemetry::{
    CombatLogRecord, CombatTelemetry, WeaponSelectionTelemetryRecord, WeaponTelemetry,
    WeaponTelemetryAggregate, WeaponTelemetryKey, WeaponTelemetryOutcome, WeaponTelemetryRecord,
    telemetry_cue_keys,
};

#[cfg(feature = "network-test")]
pub(crate) mod testing {
    pub use super::authority::{TestDummy, TestDummyFixture, TestDummyResetDeadline};
    pub use super::client::CaptureCombatCues;
}

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
#[cfg(feature = "server")]
use std::collections::HashMap;
#[cfg(feature = "server")]
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

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
use std::collections::HashSet;

#[cfg(feature = "server")]
use crate::movement::{DESTRUCTIBLE_MAP_LAYER, STATIC_MAP_LAYER};
use crate::protocol::{Fighter, NetworkEntityId, PlayerId};
#[cfg(feature = "server")]
use crate::timing::SimulationTick;
#[cfg(feature = "server")]
use crate::{
    gameplay::GameplaySet,
    movement::{
        ArenaWall, FIGHTER_LAYER, PROJECTILE_LAYER, fighter_collision_layers,
        input_should_neutralize,
    },
    protocol::FighterInput,
};

/// Stable identity of the canonical fighter shape; balance comes from the build catalog.
pub const STANDARD_FIGHTER_DEFINITION: FighterDefinitionId = FighterDefinitionId(1);
/// Stable identity of the Pulse Sidearm compatibility family; balance comes from weapon recipes.
pub const PULSE_SIDEARM_DEFINITION: WeaponDefinitionId = WeaponDefinitionId(1);
pub const SCATTER_CANNON_DEFINITION: WeaponDefinitionId = WeaponDefinitionId(2);
pub const ARC_LAUNCHER_DEFINITION: WeaponDefinitionId = WeaponDefinitionId(3);
pub const IMPACT_BLADE_DEFINITION: WeaponDefinitionId = WeaponDefinitionId(4);
pub const STICKY_BLOMB_DEFINITION: WeaponDefinitionId = WeaponDefinitionId(5);
pub const SPRAY_DEFINITION: WeaponDefinitionId = WeaponDefinitionId(6);
/// Reserved team and entity identity for the neutral practice dummy.
pub const NEUTRAL_TEAM: TeamId = TeamId(u8::MAX);
pub const DUMMY_NETWORK_ENTITY: NetworkEntityId = NetworkEntityId(0);

#[cfg(feature = "server")]
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CombatDamageSet {
    Combatants,
    Fields,
    Conditions,
    WorldTargets,
    ModeObjectives,
    EnvironmentReactions,
    Publish,
}

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

#[cfg(feature = "server")]
#[derive(Resource, Default, Debug)]
struct CombatSummaryLogged(bool);

#[cfg(feature = "server")]
#[derive(Resource, Clone, Copy, Debug, Default)]
struct CombatEvidenceMode {
    enabled: bool,
}

#[cfg(feature = "server")]
const COMBAT_CHECKPOINT_LATCH_TICKS: u16 = 600;
#[cfg(feature = "client")]
const MAX_COMBAT_SNAPSHOT_HISTORY: usize = 2048;

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
pub fn resolved_fighter_runtime(
    team_id: TeamId,
    fighter_stats: &crate::builds::ResolvedFighterStats,
    primary_weapon: &ResolvedWeapon,
) -> (FighterDefinitionId, TeamId, CurrentHealth, WeaponState) {
    (
        STANDARD_FIGHTER_DEFINITION,
        team_id,
        CurrentHealth(fighter_stats.maximum_health),
        WeaponState::ready(primary_weapon.recipe.economy.capacity()),
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

#[cfg(test)]
mod tests;

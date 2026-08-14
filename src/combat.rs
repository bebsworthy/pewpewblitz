//! The first authoritative combat slice: one direct-fire weapon, projectiles, damage, and reset.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
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
use lightyear::prelude::{InterpolationTarget, NetworkTarget, Replicate};

use crate::protocol::{Fighter, NetworkEntityId, PlayerId};
#[cfg(feature = "server")]
use crate::timing::SimulationTick;
#[cfg(feature = "server")]
use crate::{
    gameplay::GameplaySet,
    movement::{
        ArenaWall, DESTRUCTIBLE_TERRAIN_LAYER, FIGHTER_LAYER, INDESTRUCTIBLE_TERRAIN_LAYER,
        MovementTuning, PROJECTILE_LAYER, fighter_collision_layers, input_should_neutralize,
    },
    protocol::FighterInput,
};

/// The stable ID of the one code-authored fighter used by the combat sandbox.
pub const STANDARD_FIGHTER_DEFINITION: FighterDefinitionId = FighterDefinitionId(1);
/// The stable ID of the one code-authored weapon used by the combat sandbox.
pub const PULSE_SIDEARM_DEFINITION: WeaponDefinitionId = WeaponDefinitionId(1);
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
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub struct SelectedBuild {
    pub primary_weapon: WeaponDefinitionId,
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

/// Server-only projectile motion and cleanup state.
#[cfg(feature = "server")]
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ProjectileRuntime {
    pub owner_entity: Entity,
    pub velocity: Vec2,
    pub travelled: f32,
    pub expires_at_tick: u64,
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
            "muzzle" => Some(Self::Muzzle),
            "impact" => Some(Self::Impact),
            "damage" => Some(Self::Damage),
            "defeat" => Some(Self::Defeat),
            "reset" => Some(Self::Reset),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CombatCueKey {
    pub kind: CombatCueKind,
    pub event_id: CombatEventId,
}

/// Ordered presentation facts. Durable values remain replicated components.
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum CombatCue {
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
    pub close_hits: u64,
    pub mid_hits: u64,
    pub long_hits: u64,
    /// Wall-clock samples used only by the local impairment evidence harness. They are never
    /// used by gameplay or attribution.
    pub accepted_shot_timestamps: Vec<(ShotId, u128)>,
    /// Bounded authoritative cue payloads retained for deterministic/process evidence.
    pub cues: Vec<CombatCue>,
    pub records: Vec<CombatLogRecord>,
}

impl CombatTelemetry {
    pub fn record_cue(&mut self, cue: CombatCue) -> bool {
        if self.cues.len() < MAX_COMBAT_EVIDENCE_EVENTS {
            self.cues.push(cue);
            true
        } else {
            false
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
            next_shot_id: 1,
            next_event_id: 1,
        }
    }
}

impl NextCombatIds {
    pub fn allocate_shot(&mut self) -> Option<ShotId> {
        let id = self.next_shot_id;
        self.next_shot_id = id.checked_add(1)?;
        Some(ShotId(id))
    }

    pub fn allocate_event(&mut self) -> Option<CombatEventId> {
        let id = self.next_event_id;
        self.next_event_id = id.checked_add(1)?;
        Some(CombatEventId(id))
    }
}

#[cfg(feature = "server")]
fn advance_weapon_state(state: &mut WeaponState, weapon: &WeaponDefinition, tick: u64) {
    match state.phase {
        WeaponPhase::Cooldown { ready_at_tick } if tick >= ready_at_tick => {
            state.phase = WeaponPhase::Ready;
        }
        WeaponPhase::Reloading { ready_at_tick } if tick >= ready_at_tick => {
            state.ammo = weapon.magazine_capacity;
            state.phase = WeaponPhase::Ready;
        }
        _ => {}
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

#[must_use]
#[cfg(feature = "server")]
fn applied_damage(requested: u16, current_health: u16) -> u16 {
    requested.min(current_health)
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

#[cfg(feature = "server")]
#[derive(Message, Clone, Copy, Debug)]
pub struct ProjectileImpact {
    pub event_id: CombatEventId,
    pub source: ProjectileSource,
    pub target: Option<Entity>,
    pub target_network_id: Option<NetworkEntityId>,
    pub position: Vec2,
    pub normal: Vec2,
    pub travelled: f32,
    /// Fraction of the current fixed-tick sweep at which the impact occurred.
    pub impact_fraction: f32,
    pub band: DistanceBand,
    pub requested_damage: u16,
}

#[cfg(feature = "server")]
#[derive(Message, Clone, Copy, Debug)]
pub struct PendingDamage {
    pub event_id: CombatEventId,
    pub source: ProjectileSource,
    pub target: Entity,
    pub target_network_id: NetworkEntityId,
    pub requested_damage: u16,
    pub travelled: f32,
    pub impact_fraction: f32,
    pub band: DistanceBand,
}

#[cfg(feature = "server")]
#[derive(Message, Clone, Copy, Debug)]
pub struct DamageApplied {
    pub event_id: CombatEventId,
    pub source: DamageSource,
    pub target: NetworkEntityId,
    pub requested: u16,
    pub amount: u16,
    pub health_after: u16,
    pub distance_band: DistanceBand,
}

#[cfg(feature = "server")]
#[derive(Message, Clone, Copy, Debug)]
pub struct FighterDefeated {
    pub event_id: CombatEventId,
    pub source: DamageSource,
    pub target: NetworkEntityId,
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

#[cfg(feature = "server")]
fn pending_damage_order(left: &PendingDamage, right: &PendingDamage) -> std::cmp::Ordering {
    left.target_network_id
        .0
        .cmp(&right.target_network_id.0)
        .then_with(|| left.impact_fraction.total_cmp(&right.impact_fraction))
        .then_with(|| left.source.shot_id.0.cmp(&right.source.shot_id.0))
}

#[cfg(feature = "server")]
#[must_use]
fn projectile_step_distance(projectile_speed: f32, maximum_range: f32, travelled: f32) -> f32 {
    (projectile_speed / 60.0).min((maximum_range - travelled).max(0.0))
}

#[must_use]
pub fn sandbox_team(player_id: PlayerId) -> TeamId {
    TeamId(u8::try_from(player_id.0.saturating_sub(1) % 2).expect("team index fits in u8"))
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
            .init_resource::<CombatOutbox>()
            .init_resource::<CombatSummaryLogged>()
            .insert_resource(CombatEvidenceMode {
                enabled: env::var("BRAWLER_NETWORK_ASSERT_COMBAT").as_deref() == Ok("1"),
            })
            .add_message::<ProjectileImpact>()
            .add_message::<PendingDamage>()
            .add_message::<DamageApplied>()
            .add_message::<FighterDefeated>()
            .add_systems(Startup, (validate_definitions, spawn_test_dummy).chain())
            .add_systems(
                FixedUpdate,
                (
                    reset_due_fighters.in_set(GameplaySet::Lifecycle),
                    ApplyDeferred.after(GameplaySet::Lifecycle),
                    authoritative_fire.in_set(GameplaySet::Fire),
                    ApplyDeferred.after(GameplaySet::Fire),
                ),
            )
            .add_systems(
                FixedPostUpdate,
                (
                    sweep_projectiles
                        .after(avian2d::prelude::PhysicsSystems::StepSimulation)
                        .in_set(CombatSet::ProjectileSweep),
                    queue_pending_damage.in_set(CombatSet::Damage),
                    apply_pending_damage
                        .after(queue_pending_damage)
                        .in_set(CombatSet::Damage),
                    clear_defeated_projectiles.in_set(CombatSet::Lifecycle),
                    emit_combat_outcomes
                        .in_set(CombatSet::TelemetryAndCues)
                        .before(send_combat_cues),
                    send_combat_cues.in_set(CombatSet::TelemetryAndCues),
                    publish_authoritative_tick
                        .in_set(CombatSet::Finalize)
                        .before(crate::gameplay::advance_simulation_tick),
                ),
            )
            .add_systems(Update, cleanup_disconnected_projectiles)
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
    let position = Vec2::new(0.0, -300.0);
    let spawn_facing = fighter.spawn_facing;
    let body_radius = fighter.body_radius;
    let (fighter_definition, build, team, health, weapon) =
        default_fighter_runtime(NEUTRAL_TEAM, &fighters, &weapons);
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
        &Defeated,
        &SpawnState,
    )>,
) {
    for (entity, network_id, fighter_id, build, defeated, spawn) in &query {
        if !reset_is_due(tick.0, defeated.reset_at_tick) {
            continue;
        }
        let Some(fighter) = fighters.get(*fighter_id) else {
            continue;
        };
        let Some(weapon) = weapons.get(build.primary_weapon) else {
            continue;
        };
        let Some(event_id) = ids.allocate_event() else {
            continue;
        };
        let position = spawn.position;
        commands
            .entity(entity)
            .insert((
                CurrentHealth(fighter.maximum_health),
                WeaponState {
                    ammo: weapon.magazine_capacity,
                    phase: WeaponPhase::Ready,
                },
                Position::from_xy(position.x, position.y),
                Rotation::radians(spawn.facing),
                fighter_collision_layers(),
            ))
            .remove::<Defeated>();
        telemetry.records.push(CombatLogRecord::Reset {
            tick: tick.0,
            event_id,
            target: *network_id,
            position: WorldPoint::from(position),
        });
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
fn authoritative_fire(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    fighters: Res<FighterDefinitions>,
    weapons: Res<WeaponDefinitions>,
    evidence: Res<CombatEvidenceMode>,
    mut ids: ResMut<NextCombatIds>,
    mut telemetry: ResMut<CombatTelemetry>,
    mut outbox: ResMut<CombatOutbox>,
    query: Query<
        (
            Entity,
            &Position,
            &Rotation,
            &FighterDefinitionId,
            &SelectedBuild,
            &TeamId,
            &PlayerId,
            &NetworkEntityId,
            &crate::movement::InputFreshness,
            &mut WeaponState,
            Option<&ActionState<FighterInput>>,
            Option<&Defeated>,
        ),
        With<Fighter>,
    >,
) {
    for (
        entity,
        position,
        rotation,
        fighter_id,
        build,
        team,
        player_id,
        network_id,
        freshness,
        mut state,
        action,
        defeated,
    ) in query
    {
        if defeated.is_some() {
            continue;
        }
        let Some(_fighter) = fighters.get(*fighter_id) else {
            continue;
        };
        let Some(weapon) = weapons.get(build.primary_weapon) else {
            continue;
        };
        advance_weapon_state(&mut state, weapon, tick.0);
        let input = action.map_or(FighterInput::default(), |value| value.0);
        let held = !input_should_neutralize(tick.0, freshness.last_fresh_tick, 12)
            && input.is_valid()
            && input.gameplay_buttons & FighterInput::PRIMARY_FIRE != 0;
        if !held || !matches!(state.phase, WeaponPhase::Ready) {
            if held && state.ammo == 0 && matches!(state.phase, WeaponPhase::Ready) {
                state.phase = WeaponPhase::Reloading {
                    ready_at_tick: tick.0.saturating_add(weapon.reload_duration_ticks),
                };
            }
            continue;
        }
        if state.ammo == 0 {
            state.phase = WeaponPhase::Reloading {
                ready_at_tick: tick.0.saturating_add(weapon.reload_duration_ticks),
            };
            continue;
        }
        let Some(shot_id) = ids.allocate_shot() else {
            warn!(tick = tick.0, "combat shot allocator exhausted");
            continue;
        };
        let Some(event_id) = ids.allocate_event() else {
            warn!(tick = tick.0, "combat event allocator exhausted");
            continue;
        };
        state.ammo = state.ammo.saturating_sub(1);
        state.phase = if state.ammo == 0 {
            WeaponPhase::Reloading {
                ready_at_tick: tick.0.saturating_add(weapon.reload_duration_ticks),
            }
        } else {
            WeaponPhase::Cooldown {
                ready_at_tick: tick.0.saturating_add(weapon.fire_cooldown_ticks),
            }
        };
        let direction = Vec2::from_angle(rotation.as_radians());
        let muzzle = muzzle_position(position.0, rotation.as_radians(), weapon.muzzle_offset);
        let source = ProjectileSource {
            shot_id,
            player_id: *player_id,
            owner_network_entity_id: *network_id,
            team_id: *team,
            weapon_definition_id: build.primary_weapon,
        };
        commands.spawn((
            Projectile,
            source,
            ProjectileRuntime {
                owner_entity: entity,
                velocity: direction * weapon.projectile_speed,
                travelled: 0.0,
                expires_at_tick: tick.0.saturating_add(weapon.maximum_lifetime_ticks),
            },
            Position::from_xy(muzzle.x, muzzle.y),
            Rotation::radians(rotation.as_radians()),
            Collider::circle(weapon.projectile_radius),
            CollisionLayers::new(
                PROJECTILE_LAYER,
                FIGHTER_LAYER | INDESTRUCTIBLE_TERRAIN_LAYER | DESTRUCTIBLE_TERRAIN_LAYER,
            ),
            Replicate::to_clients(NetworkTarget::All),
            InterpolationTarget::to_clients(NetworkTarget::All),
        ));
        telemetry.accepted_shots = telemetry.accepted_shots.saturating_add(1);
        telemetry.records.push(CombatLogRecord::Shot {
            event_id,
            tick: tick.0,
            shot_id,
            source: *network_id,
            weapon: build.primary_weapon,
            muzzle_position: WorldPoint::from(muzzle),
            ammo_after: state.ammo,
        });
        info!(
            tick = tick.0,
            shot_id = shot_id.0,
            source = network_id.0,
            ammo_after = state.ammo,
            "authoritative pulse shot accepted"
        );
        let cue = CombatCue::Muzzle {
            event_id,
            tick: tick.0,
            source: *network_id,
            shot_id,
            weapon_definition_id: build.primary_weapon,
            position: WorldPoint::from(muzzle),
        };
        let cue_retained = telemetry.record_cue(cue.clone());
        if evidence.enabled
            && cue_retained
            && telemetry.accepted_shot_timestamps.len() < MAX_COMBAT_EVIDENCE_EVENTS
        {
            telemetry
                .accepted_shot_timestamps
                .push((shot_id, unix_epoch_micros()));
        }
        outbox.0.push(cue);
    }
}

#[cfg(feature = "server")]
#[derive(Resource, Default, Debug)]
pub struct CombatOutbox(pub Vec<CombatCue>);

#[cfg(feature = "server")]
fn sweep_projectiles(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    weapons: Res<WeaponDefinitions>,
    mut ids: ResMut<NextCombatIds>,
    mut impacts: MessageWriter<ProjectileImpact>,
    mut projectiles: Query<(Entity, &Position, &mut ProjectileRuntime, &ProjectileSource)>,
    fighters: Query<(Entity, &TeamId, Option<&Defeated>, &NetworkEntityId), With<Fighter>>,
    walls: Query<Entity, With<ArenaWall>>,
    spatial_query: avian2d::prelude::SpatialQuery,
) {
    let fighter_info: HashMap<_, _> = fighters
        .iter()
        .map(|(entity, team, defeated, network_id)| {
            (entity, (team.0, defeated.is_some(), *network_id))
        })
        .collect();
    let wall_entities: HashSet<_> = walls.iter().collect();
    let mut ordered: Vec<_> = projectiles.iter_mut().collect();
    ordered.sort_by_key(|(_, _, _, source)| source.shot_id.0);
    for (entity, position, mut runtime, source) in ordered {
        let Some(weapon) = weapons.get(source.weapon_definition_id) else {
            commands.entity(entity).despawn();
            continue;
        };
        if tick.0 >= runtime.expires_at_tick || runtime.travelled >= weapon.maximum_range {
            commands.entity(entity).despawn();
            continue;
        }
        let remaining = projectile_step_distance(
            weapon.projectile_speed,
            weapon.maximum_range,
            runtime.travelled,
        );
        let direction = runtime.velocity.normalize_or_zero();
        let Some(direction) = Dir2::new(direction).ok() else {
            commands.entity(entity).despawn();
            continue;
        };
        let filter = avian2d::prelude::SpatialQueryFilter::from_mask(
            FIGHTER_LAYER | INDESTRUCTIBLE_TERRAIN_LAYER | DESTRUCTIBLE_TERRAIN_LAYER,
        )
        .with_excluded_entities([entity, runtime.owner_entity]);
        let hit = spatial_query.cast_shape_predicate(
            &Collider::circle(weapon.projectile_radius),
            position.0,
            0.0,
            direction,
            &avian2d::prelude::ShapeCastConfig::from_max_distance(remaining),
            &filter,
            &|candidate| {
                fighter_info.get(&candidate).map_or_else(
                    || wall_entities.contains(&candidate),
                    |(team, defeated, _)| *team != source.team_id.0 && !defeated,
                )
            },
        );
        let Some(hit) = hit else {
            let next_position = position.0 + direction.as_vec2() * remaining;
            runtime.travelled += remaining;
            commands.entity(entity).insert(Position(next_position));
            if runtime.travelled >= weapon.maximum_range {
                commands.entity(entity).despawn();
            }
            continue;
        };
        let hit_distance = hit.distance.clamp(0.0, remaining);
        runtime.travelled += hit_distance;
        let travelled = runtime.travelled;
        let impact_fraction = if remaining > 0.0 {
            (hit_distance / remaining).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let Some(event_id) = ids.allocate_event() else {
            commands.entity(entity).despawn();
            continue;
        };
        let target_network_id = fighter_info
            .get(&hit.entity)
            .map(|(_, _, network_id)| *network_id);
        impacts.write(ProjectileImpact {
            event_id,
            source: *source,
            target: fighter_info.contains_key(&hit.entity).then_some(hit.entity),
            target_network_id,
            position: hit.point2,
            normal: hit.normal1,
            travelled,
            impact_fraction,
            band: distance_band(travelled),
            requested_damage: fighter_info
                .get(&hit.entity)
                .map_or(0, |_| weapon.direct_damage),
        });
        commands.entity(entity).despawn();
    }
}

#[cfg(feature = "server")]
fn queue_pending_damage(
    mut impacts: MessageReader<ProjectileImpact>,
    mut pending_damage: MessageWriter<PendingDamage>,
) {
    for impact in impacts.read() {
        let (Some(target), Some(target_network_id)) = (impact.target, impact.target_network_id)
        else {
            continue;
        };
        pending_damage.write(PendingDamage {
            event_id: impact.event_id,
            source: impact.source,
            target,
            target_network_id,
            requested_damage: impact.requested_damage,
            travelled: impact.travelled,
            impact_fraction: impact.impact_fraction,
            band: impact.band,
        });
    }
}

#[cfg(feature = "server")]
fn apply_pending_damage(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    fighters: Res<FighterDefinitions>,
    mut ids: ResMut<NextCombatIds>,
    mut pending_damage: MessageReader<PendingDamage>,
    mut damage_applied: MessageWriter<DamageApplied>,
    mut fighter_defeated: MessageWriter<FighterDefeated>,
    mut telemetry: ResMut<CombatTelemetry>,
    mut targets: Query<
        (
            &NetworkEntityId,
            &FighterDefinitionId,
            &mut CurrentHealth,
            Option<&Defeated>,
        ),
        With<Fighter>,
    >,
) {
    let mut damages: Vec<_> = pending_damage.read().copied().collect();
    damages.sort_by(pending_damage_order);
    let mut defeated_this_tick = HashSet::new();
    for damage in damages {
        let Ok((target_id, fighter_id, mut health, defeated)) = targets.get_mut(damage.target)
        else {
            continue;
        };
        let requested = damage.requested_damage;
        let applied = applied_damage(requested, health.0);
        health.0 = health.0.saturating_sub(applied);
        telemetry.hostile_fighter_hits = telemetry.hostile_fighter_hits.saturating_add(1);
        match damage.band {
            DistanceBand::Close => telemetry.close_hits = telemetry.close_hits.saturating_add(1),
            DistanceBand::Mid => telemetry.mid_hits = telemetry.mid_hits.saturating_add(1),
            DistanceBand::Long => telemetry.long_hits = telemetry.long_hits.saturating_add(1),
        }
        if applied == 0 {
            continue;
        }
        let source = DamageSource::PlayerWeapon {
            player_id: damage.source.player_id,
            fighter_id: damage.source.owner_network_entity_id,
            weapon_definition_id: damage.source.weapon_definition_id,
            shot_id: damage.source.shot_id,
        };
        let Some(damage_event) = ids.allocate_event() else {
            continue;
        };
        damage_applied.write(DamageApplied {
            event_id: damage_event,
            source,
            target: *target_id,
            requested,
            amount: applied,
            health_after: health.0,
            distance_band: damage.band,
        });
        if health.0 == 0 && defeated.is_none() && defeated_this_tick.insert(damage.target) {
            let Some(defeat_event) = ids.allocate_event() else {
                continue;
            };
            let reset_delay = fighters
                .get(*fighter_id)
                .map_or(90, |definition| definition.defeat_reset_delay_ticks);
            let reset_at_tick = tick.0.saturating_add(reset_delay);
            commands.entity(damage.target).insert(Defeated {
                event_id: defeat_event,
                reset_at_tick,
            });
            commands.entity(damage.target).insert(CollisionLayers::new(
                FIGHTER_LAYER,
                avian2d::prelude::LayerMask::NONE,
            ));
            fighter_defeated.write(FighterDefeated {
                event_id: defeat_event,
                source,
                target: *target_id,
            });
            info!(tick = tick.0, target = target_id.0, "fighter defeated");
        }
    }
}

#[cfg(feature = "server")]
fn emit_combat_outcomes(
    tick: Res<SimulationTick>,
    mut impacts: MessageReader<ProjectileImpact>,
    mut damage_applied: MessageReader<DamageApplied>,
    mut fighter_defeated: MessageReader<FighterDefeated>,
    mut telemetry: ResMut<CombatTelemetry>,
    mut outbox: ResMut<CombatOutbox>,
) {
    for impact in impacts.read() {
        let cue = CombatCue::Impact {
            event_id: impact.event_id,
            tick: tick.0,
            source: impact.source.owner_network_entity_id,
            shot_id: impact.source.shot_id,
            weapon_definition_id: impact.source.weapon_definition_id,
            target: impact.target_network_id,
            position: WorldPoint::from(impact.position),
            normal: WorldPoint::from(impact.normal),
            distance_band: impact.band,
        };
        telemetry.record_cue(cue.clone());
        outbox.0.push(cue);
        telemetry.records.push(CombatLogRecord::Hit {
            tick: tick.0,
            event_id: impact.event_id,
            shot_id: impact.source.shot_id,
            source: impact.source.owner_network_entity_id,
            target: impact.target_network_id,
            weapon: impact.source.weapon_definition_id,
            position: WorldPoint::from(impact.position),
            distance: impact.travelled,
            band: impact.band,
        });
    }
    for damage in damage_applied.read() {
        telemetry.applied_damage = telemetry
            .applied_damage
            .saturating_add(u64::from(damage.amount));
        let cue = CombatCue::Damage {
            event_id: damage.event_id,
            tick: tick.0,
            source: damage.source,
            target: damage.target,
            amount: damage.amount,
            health_after: damage.health_after,
            distance_band: damage.distance_band,
        };
        telemetry.record_cue(cue.clone());
        outbox.0.push(cue);
        telemetry.records.push(CombatLogRecord::Damage {
            tick: tick.0,
            event_id: damage.event_id,
            source: damage.source,
            target: damage.target,
            requested: damage.requested,
            applied: damage.amount,
            health_after: damage.health_after,
        });
    }
    for defeat in fighter_defeated.read() {
        telemetry.defeats = telemetry.defeats.saturating_add(1);
        telemetry.records.push(CombatLogRecord::Defeat {
            tick: tick.0,
            event_id: defeat.event_id,
            source: Some(defeat.source),
            target: defeat.target,
        });
        let cue = CombatCue::Defeat {
            event_id: defeat.event_id,
            tick: tick.0,
            source: Some(defeat.source),
            target: defeat.target,
        };
        telemetry.record_cue(cue.clone());
        outbox.0.push(cue);
    }
}

#[cfg(feature = "server")]
fn clear_defeated_projectiles() {}

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
    disconnected: Query<
        Entity,
        (
            With<lightyear::prelude::LinkOf>,
            With<lightyear::prelude::Disconnected>,
        ),
    >,
    fighters: Query<(Entity, &lightyear::prelude::ControlledBy), With<Fighter>>,
    projectiles: Query<(Entity, &ProjectileRuntime)>,
) {
    let disconnected: HashSet<_> = disconnected.iter().collect();
    let mut fighter_entities = HashSet::new();
    let mut disconnected_fighters = HashSet::new();
    for (fighter, controlled) in &fighters {
        fighter_entities.insert(fighter);
        if disconnected.contains(&controlled.owner) {
            disconnected_fighters.insert(fighter);
        }
    }
    for (entity, projectile) in &projectiles {
        if disconnected_fighters.contains(&projectile.owner_entity)
            || !fighter_entities.contains(&projectile.owner_entity)
        {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(feature = "server")]
fn send_combat_cues(
    mut outbox: ResMut<CombatOutbox>,
    mut senders: Query<
        &mut lightyear::prelude::MessageSender<CombatCue>,
        With<lightyear::prelude::LinkOf>,
    >,
) {
    let cues = std::mem::take(&mut outbox.0);
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
#[derive(Component)]
pub struct CombatHudText;

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
                    ensure_projectile_visuals,
                    sync_projectile_visuals,
                    update_health_bars,
                    update_combat_hud,
                    update_combat_effects,
                    record_headless_combat_observation,
                )
                    .chain(),
            );
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
                CombatCue::Defeat { .. } => observation.saw_defeat = true,
                CombatCue::Reset { .. } => observation.saw_reset = true,
                _ => {}
            }
            let event_id = match &cue {
                CombatCue::Muzzle { event_id, .. }
                | CombatCue::Impact { event_id, .. }
                | CombatCue::Damage { event_id, .. }
                | CombatCue::Defeat { event_id, .. }
                | CombatCue::Reset { event_id, .. } => *event_id,
            };
            if !remember_combat_event(&mut recent, event_id) {
                continue;
            }
            if let Some(capture) = capture.as_mut()
                && capture.cues.len() < MAX_COMBAT_EVIDENCE_EVENTS
            {
                capture.cues.push(cue.clone());
            }
            if observation.ready_file.is_some() {
                if observation.cue_stream.len() < MAX_COMBAT_EVIDENCE_EVENTS {
                    observation.cue_stream.push(cue.clone());
                }
                if let CombatCue::Muzzle { shot_id, .. } = &cue
                    && observation.cue_timestamps.len() < MAX_COMBAT_EVIDENCE_EVENTS
                {
                    observation
                        .cue_timestamps
                        .push((*shot_id, unix_epoch_micros()));
                }
            }
            let target_position = match &cue {
                CombatCue::Damage { target, .. } | CombatCue::Defeat { target, .. } => fighters
                    .iter()
                    .find(|(network_id, _)| **network_id == *target)
                    .map(|(_, position)| position.0),
                _ => None,
            };
            let local_hit = match &cue {
                CombatCue::Damage {
                    source: DamageSource::PlayerWeapon { player_id, .. },
                    ..
                } => local_player == Some(*player_id),
                _ => false,
            };
            let (position, color, size) = match cue {
                CombatCue::Muzzle { position, .. } => (
                    position.as_vec2(),
                    Color::srgb(1.0, 0.8, 0.2),
                    Vec2::splat(22.0),
                ),
                CombatCue::Impact { position, .. } => (
                    position.as_vec2(),
                    Color::srgb(1.0, 0.35, 0.1),
                    Vec2::splat(28.0),
                ),
                CombatCue::Damage { .. } => (
                    target_position.unwrap_or(Vec2::ZERO),
                    if local_hit {
                        Color::srgb(1.0, 0.9, 0.2)
                    } else {
                        Color::srgb(1.0, 0.1, 0.1)
                    },
                    Vec2::splat(18.0),
                ),
                CombatCue::Defeat { .. } => (
                    target_position.unwrap_or(Vec2::ZERO),
                    Color::srgb(0.9, 0.05, 0.05),
                    Vec2::splat(64.0),
                ),
                CombatCue::Reset { position, .. } => (
                    position.as_vec2(),
                    Color::srgb(0.2, 1.0, 0.4),
                    Vec2::splat(42.0),
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
fn record_headless_combat_observation(
    mut observation: ResMut<ClientCombatObservation>,
    definitions: Res<FighterDefinitions>,
    weapons: Res<WeaponDefinitions>,
    fighters: Query<
        (
            &NetworkEntityId,
            &CurrentHealth,
            &WeaponState,
            &FighterDefinitionId,
            Option<&Defeated>,
        ),
        With<Fighter>,
    >,
) {
    let Some(path) = observation.ready_file.clone() else {
        return;
    };
    if observation.wrote_ready || !observation.saw_defeat || !observation.saw_reset {
        return;
    }
    let Some((_, health, weapon_state, fighter_definition_id, defeated)) = fighters
        .iter()
        .find(|(network_id, _, _, _, _)| network_id.0 == DUMMY_NETWORK_ENTITY.0)
    else {
        return;
    };
    let Some(weapon) = weapons.get(PULSE_SIDEARM_DEFINITION) else {
        return;
    };
    let Some(fighter) = definitions.get(*fighter_definition_id) else {
        return;
    };
    if health.0 != fighter.maximum_health
        || weapon_state.ammo != weapon.magazine_capacity
        || !matches!(weapon_state.phase, WeaponPhase::Ready)
        || defeated.is_some()
    {
        return;
    }
    let mut report = format!(
        "client_elapsed_ms={}\n",
        observation.started_at.elapsed().as_millis()
    );
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
fn ensure_projectile_visuals(
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            Option<&Transform>,
            Option<&mut Sprite>,
            Option<&ProjectileSource>,
        ),
        With<Projectile>,
    >,
) {
    for (entity, transform, sprite, source) in &mut query {
        if transform.is_none() {
            commands.entity(entity).insert(Transform::default());
        }
        let color = source.map_or(Color::srgb(1.0, 0.85, 0.2), |source| {
            projectile_color(source.player_id)
        });
        if let Some(mut sprite) = sprite {
            sprite.color = color;
        } else {
            commands.entity(entity).insert((
                Sprite::from_color(color, Vec2::new(20.0, 8.0)),
                Name::new("Pulse projectile"),
            ));
        }
    }
}

#[cfg(feature = "client")]
fn sync_projectile_visuals(
    mut query: Query<(&Position, &Rotation, &mut Transform), With<Projectile>>,
) {
    for (position, rotation, mut transform) in &mut query {
        transform.translation.x = position.0.x;
        transform.translation.y = position.0.y;
        transform.translation.z = 20.0;
        transform.rotation = Quat::from_rotation_z(rotation.as_radians());
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
            Option<&Defeated>,
        ),
        (With<Fighter>, With<lightyear::prelude::Controlled>),
    >,
    weapons: Res<WeaponDefinitions>,
) {
    let Some((player_id, health, state, authoritative_tick, build, defeated)) =
        fighter.iter().next()
    else {
        return;
    };
    let weapon_id = build.map_or(PULSE_SIDEARM_DEFINITION, |build| build.primary_weapon);
    let capacity = weapons
        .get(weapon_id)
        .map_or(0, |weapon| weapon.magazine_capacity);
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
    for mut value in &mut text {
        **value = format!(
            "Player {}   Health {:>3}   Pulse {}/{}   {}",
            player_id.0, health.0, state.ammo, capacity, phase
        );
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
        let weapon = WeaponDefinitions::default().entries[0];
        let mut state = WeaponState {
            ammo: 1,
            phase: WeaponPhase::Ready,
        };
        state.ammo -= 1;
        state.phase = WeaponPhase::Reloading { ready_at_tick: 61 };
        assert_eq!(state.ammo, 0);
        advance_weapon_state(&mut state, &weapon, 60);
        assert_eq!(state.ammo, 0);
        assert_eq!(state.phase, WeaponPhase::Reloading { ready_at_tick: 61 });
        advance_weapon_state(&mut state, &weapon, 61);
        assert_eq!(state.ammo, weapon.magazine_capacity);
        assert_eq!(state.phase, WeaponPhase::Ready);
        state.phase = WeaponPhase::Cooldown { ready_at_tick: 73 };
        advance_weapon_state(&mut state, &weapon, 72);
        assert_eq!(state.phase, WeaponPhase::Cooldown { ready_at_tick: 73 });
        advance_weapon_state(&mut state, &weapon, 73);
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

    #[cfg(feature = "server")]
    #[test]
    fn pending_damage_order_uses_current_sweep_fraction_before_shot_id() {
        let source = |shot_id| ProjectileSource {
            shot_id: ShotId(shot_id),
            player_id: PlayerId(1),
            owner_network_entity_id: NetworkEntityId(1),
            team_id: TeamId(0),
            weapon_definition_id: PULSE_SIDEARM_DEFINITION,
        };
        let mut damages = [
            PendingDamage {
                event_id: CombatEventId(1),
                source: source(1),
                target: Entity::PLACEHOLDER,
                target_network_id: NetworkEntityId(9),
                requested_damage: 25,
                travelled: 810.0,
                impact_fraction: 0.9,
                band: DistanceBand::Long,
            },
            PendingDamage {
                event_id: CombatEventId(2),
                source: source(2),
                target: Entity::PLACEHOLDER,
                target_network_id: NetworkEntityId(9),
                requested_damage: 25,
                travelled: 30.0,
                impact_fraction: 0.1,
                band: DistanceBand::Close,
            },
        ];

        damages.sort_by(pending_damage_order);

        assert_eq!(damages[0].source.shot_id, ShotId(2));
        assert_eq!(damages[1].source.shot_id, ShotId(1));
    }

    #[cfg(feature = "server")]
    #[test]
    fn projectile_step_distance_accumulates_and_clamps_the_final_step() {
        let mut travelled = 0.0;
        for _ in 0..2 {
            travelled += projectile_step_distance(900.0, 35.0, travelled);
        }
        assert!((travelled - 30.0).abs() < f32::EPSILON);
        travelled += projectile_step_distance(900.0, 35.0, travelled);
        assert!((travelled - 35.0).abs() < f32::EPSILON);
        assert!(projectile_step_distance(900.0, 35.0, travelled).abs() < f32::EPSILON);
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

    #[cfg(feature = "server")]
    #[test]
    fn applied_damage_clamps_overkill_without_underflow() {
        assert_eq!(applied_damage(25, 10), 10);
        assert_eq!(applied_damage(0, 10), 0);
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
